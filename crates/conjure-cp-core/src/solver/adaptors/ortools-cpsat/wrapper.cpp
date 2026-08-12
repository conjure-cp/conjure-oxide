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

rust::Vec<uint8_t> solve_wrapper(rust::Slice<const uint8_t> model_proto_bytes,
                                 size_t callback_ptr,
                                 bool enumerate_all,
                                 size_t num_decision_vars) {
  using namespace operations_research;
  using namespace operations_research::sat;

  std::cerr << "[wrapper] Entering solve_wrapper. enumerate_all: " << enumerate_all << std::endl;

  sat::CpModelProto model_proto;

  if (!model_proto.ParseFromArray(model_proto_bytes.data(),
                                  model_proto_bytes.size())) {
    std::cerr << "[wrapper] Failed to parse model proto!" << std::endl;
    return {};
  }

  std::cerr << "[wrapper] Parsed model proto." << std::endl;

  sat::Model model;
  sat::SatParameters parameters;
  parameters.set_enumerate_all_solutions(enumerate_all);
  if (model_proto.search_strategy_size() > 0) {
    parameters.set_search_branching(sat::SatParameters::FIXED_SEARCH);
  } else {
    parameters.set_search_branching(sat::SatParameters::AUTOMATIC_SEARCH);
  }
  // Keep a reasonable memory limit just in case
  parameters.set_max_memory_in_mb(1024);
  parameters.set_num_search_workers(1);
  parameters.set_random_seed(1);
  parameters.set_permute_variable_randomly(false);
  parameters.set_permute_presolve_constraint_order(false);
  model.Add(NewSatParameters(parameters));

  std::cerr << "[wrapper] Configured parameters. enumerate_all: " << parameters.enumerate_all_solutions() << std::endl;

  std::atomic<bool> stopped(false);
  model.GetOrCreate<TimeLimit>()->RegisterExternalBooleanAsLimit(&stopped);

  int sol_count = 0;
  model.Add(NewFeasibleSolutionObserver([&](const sat::CpSolverResponse &r) {
    sol_count++;
    sat::CpSolverResponse filtered_r = r;
    if (num_decision_vars > 0 && static_cast<size_t>(filtered_r.solution_size()) > num_decision_vars) {
      filtered_r.mutable_solution()->Truncate(num_decision_vars);
    }
    std::vector<uint8_t> serialized(filtered_r.ByteSizeLong());
    if (filtered_r.SerializeToArray(serialized.data(), serialized.size())) {
      rust::Slice<const uint8_t> slice(serialized.data(), serialized.size());
      bool ret = invoke_callback(callback_ptr, slice);
      if (!ret) {
        stopped = true;
      }
    }
  }));

  std::cerr << "[wrapper] Calling sat::SolveCpModel..." << std::endl;
  sat::CpSolverResponse final_response = sat::SolveCpModel(model_proto, &model);
  std::cerr << "[wrapper] sat::SolveCpModel returned. Status: " << final_response.status() << std::endl;

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

  std::cerr << "[wrapper] Returning output." << std::endl;
  return output;
}
