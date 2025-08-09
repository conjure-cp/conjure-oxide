import json
import os
from glob import glob

print("Running collect_stats")
perform_dir = './tools/performance/data'
rows=[]
for rewrite_cat in os.listdir(perform_dir):
    rewrite_cat_path = os.path.join(perform_dir, rewrite_cat)
    if os.path.isdir(rewrite_cat_path):
        filename = f"{rewrite_cat_path}/{rewrite_cat}.csv" 
        try:
            os.remove(filename)
        except OSError:
            pass
        csv = open(filename, 'a')
        csv.write("test,solver,solver_time,solver_nodes,rewriter_time,total_time")
        for test_dir in sorted(os.listdir(rewrite_cat_path)):
            test_dir_path = os.path.join(rewrite_cat_path, test_dir)
            if os.path.isdir(test_dir_path):
                oxide_stats = glob(os.path.join(test_dir_path, 'oxide-stats.json'))
                conjure_stats = glob(os.path.join(test_dir_path, '*.stats.json'))
                if oxide_stats:
                    with open(oxide_stats[0], 'r') as f:
                        oxide_data = json.load(f)
                        solver_stats = oxide_data.get('stats', {}).get('solverRuns', [])[0]
                        rewrite_stats = oxide_data.get('stats', {}).get('rewriterRuns', [])[0]
                        solver = solver_stats.get('solverAdaptor', 0)
                        solver_time = round(float(solver_stats.get('conjureSolverWallTime_s', 0)),4)
                        solver_nodes = solver_stats.get('nodes', 0)
                        rewriter_runtime = rewrite_stats.get('rewriterRunTime',[])
                        rewriter_time = round(float((rewriter_runtime.get('secs',0)) + (rewriter_runtime.get('nanos',0)/10**9)),4)
                        total_time = round(float(rewriter_time + solver_time),4)
                        csv.write("\n" + test_dir + "," + solver + "," + str(solver_time) + "," + str(solver_nodes) + "," + str(rewriter_time) + "," + str(total_time))
                if conjure_stats:
                    with open(conjure_stats[0], 'r') as f:
                        conjure_data = json.load(f)
                        savile_row_stats = conjure_data.get('savilerowInfo', 0)
                        solver = conjure_data.get('solver', "")
                        solver_nodes = savile_row_stats.get('SolverNodes', 0)
                        rewriter_time = round(float(savile_row_stats.get('SavileRowTotalTime', 0)),4)
                        solver_time = round(float(savile_row_stats.get('SolverTotalTime', 0)),4)
                        total_time = round((rewriter_time + solver_time),4)
                        csv.write("\n" + test_dir + "," + solver + "," + str(solver_time) + "," + str(solver_nodes) + "," + str(rewriter_time) + "," + str(total_time))