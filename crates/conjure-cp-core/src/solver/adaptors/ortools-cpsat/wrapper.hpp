#pragma once
#include "rust/cxx.h"
#include <cstdint>

rust::Vec<uint8_t> solve_wrapper(rust::Slice<const uint8_t> model_proto, size_t callback_ptr, bool enumerate_all, size_t num_decision_vars);
