#include "chuffed/branching/branching.h"
#include "chuffed/core/engine.h"
#include "chuffed/core/propagator.h"
#include "chuffed/flatzinc/flatzinc.h"
#include "chuffed/primitives/primitives.h"
#include "chuffed/vars/modelling.h"

class DummyProblem {
public:
  vec<IntVar *> *searchVars;
  // callback
  void (*callback)(vec<IntVar *> *);

  void print() { callback(searchVars); }
  void setcallback(void (*_callback)(vec<IntVar *> *)) { callback = _callback; }
  void addVars(vec<IntVar *> *_searchVars) { searchVars = _searchVars; }
};

DummyProblem *new_dummy_problem();
void p_addVars(DummyProblem *p, vec<IntVar *> *_searchVars);
void p_setcallback(DummyProblem *p, void (*_callback)(vec<IntVar *> *));
void p_print(DummyProblem *p);
int get_idx(vec<IntVar *> *x, int i);

vec<IntVar *> *make_vec_intvar();
void destroy_vec_intvar(vec<IntVar *> *v);

void branch_IntVar(vec<IntVar *> *x, VarBranch var_branch,
                   ValBranch val_branch);

Problem *new_problem();
void solve(Problem *p);
