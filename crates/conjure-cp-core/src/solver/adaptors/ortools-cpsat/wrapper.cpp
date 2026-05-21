#include "wrapper.hpp"
#include "ortools/base/base_export.h"
#ifndef OR_PROTO_DLL
#define OR_PROTO_DLL OR_DLL
#endif
#include "ortools/sat/cp_model_solver.h" 
#include "ortools/sat/cp_model.h"
#include "ortools/sat/model.h"
#include "ortools/sat/sat_parameters.pb.h"

#include <vector>

rust::Vec<uint8_t> solve_wrapper(rust::Slice<const uint8_t> model_proto_bytes) {
    using namespace operations_research;
    using namespace operations_research::sat;

    sat::CpModelProto model_proto;
    
    if (!model_proto.ParseFromArray(model_proto_bytes.data(), model_proto_bytes.size())) {
        return {};
    }

    sat::Model model;
    sat::SatParameters parameters;
    parameters.set_enumerate_all_solutions(true);
    model.Add(NewSatParameters(parameters));

    std::vector<sat::CpSolverResponse> all_responses;

    model.Add(NewFeasibleSolutionObserver([&](const sat::CpSolverResponse& r) {
        all_responses.push_back(r);
    }));

    const sat::CpSolverResponse final_response = sat::SolveCpModel(model_proto, &model);
    all_responses.push_back(final_response);

    rust::Vec<uint8_t> output;
    for (const auto& response : all_responses) {
        std::vector<uint8_t> serialized(response.ByteSizeLong());
        if (!response.SerializeToArray(serialized.data(), serialized.size())) {
            continue;
        }
        uint32_t len = serialized.size();
        output.push_back(len & 0xFF);
        output.push_back((len >> 8) & 0xFF);
        output.push_back((len >> 16) & 0xFF);
        output.push_back((len >> 24) & 0xFF);
        for (uint8_t byte : serialized) {
            output.push_back(byte);
        }
    }

    return output;
}
