#include "chuffed/branching/branching.h"
#include "chuffed/core/engine.h"
#include "chuffed/core/propagator.h"
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
IntVar get_idx(vec<IntVar *> *x, int i);

void branch_IntVar(vec<IntVar *> *x, VarBranch var_branch,
                   ValBranch val_branch);
