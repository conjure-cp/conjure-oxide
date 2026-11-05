#include "wrapper.hpp"
#include "ortools/base/base_export.h"
#ifndef OR_PROTO_DLL
#define OR_PROTO_DLL OR_DLL
#endif
#include "ortools/sat/cp_model.h"
#include "ortools/sat/cp_model_solver.h"
#include "ortools/sat/model.h"
#include "ortools/sat/sat_parameters.pb.h"

#include "conjure-cp-core/src/solver/adaptors/ortools-cpsat/mod.rs.h" // For invoke_callback
#include "ortools/util/time_limit.h"
#include <atomic>
#include <iostream>
#include <limits>
#include <set>

rust::Vec<uint8_t> solve_wrapper(rust::Slice<const uint8_t> model_proto_bytes,
                                 size_t callback_ptr,
                                 bool enumerate_all,
                                 rust::Slice<const size_t> decision_vars) {
  using namespace operations_research;
  using namespace operations_research::sat;

  sat::CpModelProto model_proto;
  if (!model_proto.ParseFromArray(model_proto_bytes.data(),
                                  model_proto_bytes.size())) {
    return {};
  }

  if (enumerate_all && !decision_vars.empty()) {
    sat::SatParameters parameters;
    parameters.set_enumerate_all_solutions(false);
    parameters.set_max_memory_in_mb(1024);
    parameters.set_random_seed(1);
    parameters.set_num_search_workers(1);
    if (model_proto.search_strategy_size() > 0) {
      parameters.set_search_branching(sat::SatParameters::FIXED_SEARCH);
    } else {
      parameters.set_search_branching(sat::SatParameters::AUTOMATIC_SEARCH);
    }

    // Use a single negated table constraint to block found assignments on decision variables
    // without inflating the model with auxiliary variables or extra linear constraints.
    auto* table_ct = model_proto.add_constraints();
    auto* forbidden_table = table_ct->mutable_table();
    forbidden_table->set_negated(true);
    for (size_t dec_idx : decision_vars) {
      forbidden_table->add_vars(static_cast<int>(dec_idx));
    }

    sat::CpSolverResponse final_response;

    while (true) {
      sat::Model model;
      model.Add(NewSatParameters(parameters));
      sat::CpSolverResponse response = sat::SolveCpModel(model_proto, &model);

      if (response.status() != sat::CpSolverStatus::OPTIMAL &&
          response.status() != sat::CpSolverStatus::FEASIBLE) {
        final_response = response;
        break;
      }

      std::vector<uint8_t> serialized(response.ByteSizeLong());
      if (response.SerializeToArray(serialized.data(), serialized.size())) {
        rust::Slice<const uint8_t> slice(serialized.data(), serialized.size());
        bool ret = invoke_callback(callback_ptr, slice);
        if (!ret) {
          final_response = response;
          break;
        }
      }

      // Append found values to the forbidden table
      for (size_t dec_idx : decision_vars) {
        int var = static_cast<int>(dec_idx);
        int64_t val = (var < response.solution_size()) ? response.solution(var) : 0;
        forbidden_table->add_values(val);
      }
    }

    final_response.clear_solution();
    final_response.clear_additional_solutions();
    final_response.clear_tightened_variables();
    final_response.clear_sufficient_assumptions_for_infeasibility();

    rust::Vec<uint8_t> output;
    std::vector<uint8_t> serialized(final_response.ByteSizeLong());
    if (final_response.SerializeToArray(serialized.data(), serialized.size())) {
      output.reserve(serialized.size());
      for (uint8_t byte : serialized) {
        output.push_back(byte);
      }
    }
    return output;
  }

  sat::Model model;
  sat::SatParameters parameters;
  if (model_proto.search_strategy_size() > 0) {
    parameters.set_search_branching(sat::SatParameters::FIXED_SEARCH);
  } else {
    parameters.set_search_branching(sat::SatParameters::AUTOMATIC_SEARCH);
  }
  parameters.set_max_memory_in_mb(1024);
  parameters.set_random_seed(1);
  parameters.set_num_search_workers(1);
  if (!enumerate_all && !model_proto.has_objective()) {
    parameters.set_stop_after_first_solution(true);
  }
  model.Add(NewSatParameters(parameters));

  std::atomic<bool> stopped(false);
  model.GetOrCreate<TimeLimit>()->RegisterExternalBooleanAsLimit(&stopped);

  model.Add(NewFeasibleSolutionObserver([&](const sat::CpSolverResponse &r) {
    if (stopped.load()) {
      return;
    }
    std::vector<uint8_t> serialized(r.ByteSizeLong());
    if (r.SerializeToArray(serialized.data(), serialized.size())) {
      rust::Slice<const uint8_t> slice(serialized.data(), serialized.size());
      bool ret = invoke_callback(callback_ptr, slice);
      if (!ret) {
        stopped.store(true);
      }
    }
  }));

  sat::CpSolverResponse final_response = sat::SolveCpModel(model_proto, &model);

  final_response.clear_solution();
  final_response.clear_additional_solutions();
  final_response.clear_tightened_variables();
  final_response.clear_sufficient_assumptions_for_infeasibility();

  rust::Vec<uint8_t> output;
  std::vector<uint8_t> serialized(final_response.ByteSizeLong());
  if (final_response.SerializeToArray(serialized.data(), serialized.size())) {
    output.reserve(serialized.size());
    for (uint8_t byte : serialized) {
      output.push_back(byte);
    }
  }
  return output;
}
