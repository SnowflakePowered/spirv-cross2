#include "spirv_cross_c.h"
#include <stdint.h>

void spvc_rs_expose_set(spvc_set set, uint32_t* out, size_t* length);

spvc_bool spvc_rs_constant_is_scalar(spvc_constant constant);

uint32_t spvc_rs_constant_get_vecsize(spvc_constant constant);

uint32_t spvc_rs_constant_get_matrix_colsize(spvc_constant constant);

spvc_result spvc_rs_compiler_variable_get_type(spvc_compiler compiler, spvc_variable_id variable_id, spvc_type_id* out);

spvc_bool spvc_rs_type_is_pointer(spvc_type type);

spvc_bool spvc_rs_type_is_forward_pointer(spvc_type type);

void spvc_rs_compiler_get_execution_model_indirect(spvc_compiler compiler, SpvExecutionModel* out);