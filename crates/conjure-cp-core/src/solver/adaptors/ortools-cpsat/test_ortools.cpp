#include "ortools/base/base_export.h"
#include "ortools/sat/cp_model_solver.h" 
#include "ortools/sat/cp_model.h"
#include "ortools/sat/model.h"
#include "ortools/sat/sat_parameters.pb.h"
#include "ortools/util/time_limit.h"
#include <atomic>

using namespace operations_research;

int main() {
    sat::Model model;
    std::atomic<bool> stopped(false);
    model.GetOrCreate<TimeLimit>()->RegisterExternalBooleanAsLimit(&stopped);
    stopped = true;
    return 0;
}
