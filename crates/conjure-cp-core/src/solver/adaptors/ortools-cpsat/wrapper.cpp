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

rust::Vec<uint8_t> solve_wrapper(rust::Slice<const uint8_t> model_proto_bytes,
                                 size_t callback_ptr) {
  using namespace operations_research;
  using namespace operations_research::sat;

  sat::CpModelProto model_proto;

  if (!model_proto.ParseFromArray(model_proto_bytes.data(),
                                  model_proto_bytes.size())) {
    return {};
  }

  sat::Model model;
  sat::SatParameters parameters;
  parameters.set_enumerate_all_solutions(true);
  // Keep a reasonable memory limit just in case
  parameters.set_max_memory_in_mb(1024);
  model.Add(NewSatParameters(parameters));

  std::atomic<bool> stopped(false);
  model.GetOrCreate<TimeLimit>()->RegisterExternalBooleanAsLimit(&stopped);

  model.Add(NewFeasibleSolutionObserver([&](const sat::CpSolverResponse &r) {
    std::vector<uint8_t> serialized(r.ByteSizeLong());
    if (r.SerializeToArray(serialized.data(), serialized.size())) {
      rust::Slice<const uint8_t> slice(serialized.data(), serialized.size());
      if (!invoke_callback(callback_ptr, slice)) {
        stopped = true;
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
