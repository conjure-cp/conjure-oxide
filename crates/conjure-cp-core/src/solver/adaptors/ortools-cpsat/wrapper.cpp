#include "wrapper.hpp"
#include "ortools/base/base_export.h"
#ifndef OR_PROTO_DLL
#define OR_PROTO_DLL OR_DLL
#endif
#include "ortools/sat/cp_model_solver.h" 
#include "ortools/sat/cp_model.h"

#include <vector>

rust::Vec<uint8_t> solve_wrapper(rust::Slice<const uint8_t> model_proto_bytes) {
    using namespace operations_research;
    using namespace operations_research::sat;

    sat::CpModelProto model_proto;
    
    if (!model_proto.ParseFromArray(model_proto_bytes.data(), model_proto_bytes.size())) {
        return {};
    }

    const sat::CpSolverResponse response = sat::Solve(model_proto);

    std::vector<uint8_t> serialized(response.ByteSizeLong());
    if (!response.SerializeToArray(serialized.data(), serialized.size())) {
        return {};
    }

    rust::Vec<uint8_t> output;
    output.reserve(serialized.size());
    for (uint8_t byte : serialized) {
        output.push_back(byte);
    }

    return output;
}
