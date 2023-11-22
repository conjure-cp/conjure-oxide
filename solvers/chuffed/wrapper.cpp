#include "./wrapper.h"
#include "chuffed/flatzinc/flatzinc.h"

DummyProblem *new_dummy_problem() { return new DummyProblem(); }
void p_addVars(DummyProblem *p, vec<IntVar *> *_searchVars) {
  p->addVars(_searchVars);
}
void p_setcallback(DummyProblem *p, void (*_callback)(vec<IntVar *> *)) {
  p->setcallback(_callback);
}
void p_print(DummyProblem *p) { p->print(); }

int get_idx(vec<IntVar *> *x, int i) {
  IntVar *var = *x[i];
  int t = var->getVal();
  return t;
}

vec<IntVar *> *make_vec_intvar() { return new vec<IntVar *>(); }

void destroy_vec_intvar(vec<IntVar *> *v) { delete v; }

void branch_IntVar(vec<IntVar *> *x, VarBranch var_branch,
                   ValBranch val_branch) {
  branch(*x, var_branch, val_branch);
}

// Construct problem with given number of variables
FlatZinc::FlatZincSpace *new_flat_zinc_space(int intVars, int boolVars,
                                             int setVars) {
  return new FlatZinc::FlatZincSpace(intVars, boolVars, setVars);
}

// add new int var
void addIntVar(FlatZinc::FlatZincSpace flat_zinc_space,
               FlatZinc::IntVarSpec *vs, const std::string &name) {
  flat_zinc_space.newIntVar(vs, name);
}

class XYZProblem : public Problem {

public:
  // Constants
  int n; // number of variables

  // Variables
  vec<IntVar *> x;
  vec<IntVar *> y;
  vec<IntVar *> z;

  XYZProblem(int _n) : n(_n) {
    // Create vars
    createVars(x, n, 1, 3);
    createVars(y, n, 1, 3);
    createVars(z, n, 1, 3);

    // Post constraints
    // find x, y, z : int(1..3)
    // such that x + y = z

    for (int i = 0; i < n; i++) {
      int_plus(x[i], y[i], z[i]);
    }

    // Branching
    branch(x, VAR_INORDER, VAL_MIN);
    branch(y, VAR_INORDER, VAL_MIN);
    branch(z, VAR_INORDER, VAL_MIN);

    // Declare output variables
    output_vars(x);
    output_vars(y);
    output_vars(z);
  }

  // Function to print out solution
  void print(std::ostream &out) override {
    out << "x = ";
    for (int i = 0; i < n; i++) {
      out << x[i]->getVal() << " ";
    }
    out << std::endl;
    out << "y = ";
    for (int i = 0; i < n; i++) {
      out << y[i]->getVal() << " ";
    }
    out << std::endl;
    out << "z = ";
    for (int i = 0; i < n; i++) {
      out << z[i]->getVal() << " ";
    }
    out << std::endl;
  }
};

// Create new problem
void *new_problem() { return new XYZProblem(3); }

// Solve problem
void solve(void *p) { engine.solve((XYZProblem *)p); }
