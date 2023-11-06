#include "./wrapper.h"

DummyProblem *new_dummy_problem() { return new DummyProblem(); }
void p_addVars(DummyProblem *p, vec<IntVar *> *_searchVars) {
  p->addVars(_searchVars);
}
void p_setcallback(DummyProblem *p, void (*_callback)(vec<IntVar *> *)) {
  p->setcallback(_callback);
}
IntVar* get_idx(vec<IntVar *> *x, int i) { return *x[i]; }

vec<IntVar*>* make_vec_intvar() {
  return new vec<IntVar*>();
}

void destroy_vec_intvar(vec<IntVar*>* v) {
  delete v;
}

void branch_IntVar(vec<IntVar *> *x, VarBranch var_branch,
                   ValBranch val_branch) {
  branch(*x, var_branch, val_branch);
}
