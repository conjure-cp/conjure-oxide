import json
import os
import matplotlib.pyplot as plt

base_file_path_opt = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..", "..", "..",
    "target", "criterion", "opt_factorial_n", "opt_factorial_k", "new", "estimates.json"
)

base_file_path_unop = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..", "..", "..",
    "target", "criterion", "unopt_factorial_n", "unopt_factorial_k", "new", "estimates.json"
)

n_values = list(range(1, 12))
mean_estimates_opt = []
mean_estimates_unop = []

for k in range(1, 12):
    k_path_opt = base_file_path_opt.replace("opt_factorial_k", f"opt_factorial_{k}")
    k_path_unop = base_file_path_unop.replace("unopt_factorial_k", f"unopt_factorial_{k}")
    try:
        with open(k_path_opt, 'r') as f:
            data = json.load(f)
            if 'mean' in data and 'point_estimate' in data['mean']:
                mean_value_opt = data['mean']['point_estimate']
                mean_estimates_opt.append(mean_value_opt)
            else:
                mean_estimates_opt.append(None)
    except FileNotFoundError:
        mean_estimates_opt.append(None)
    except json.JSONDecodeError:
        mean_estimates_opt.append(None)
    except Exception as e:
        mean_estimates_opt.append(None)

    try:
        with open(k_path_unop, 'r') as f:
            data = json.load(f)
            if 'mean' in data and 'point_estimate' in data['mean']:
                mean_value_unop = data['mean']['point_estimate']
                mean_estimates_unop.append(mean_value_unop)
            else:
                mean_estimates_unop.append(None)
    except FileNotFoundError:
        mean_estimates_unop.append(None)
    except json.JSONDecodeError:
        mean_estimates_unop.append(None)
    except Exception as e:
        mean_estimates_unop.append(None)

mean_estimates_opt_in_seconds = [value * 10**-9 if value is not None else None for value in mean_estimates_opt]
mean_estimates_unop_in_seconds = [value * 10**-9 if value is not None else None for value in mean_estimates_unop]

plt.plot(n_values, mean_estimates_opt_in_seconds, label="Optimised")
plt.plot(n_values, mean_estimates_unop_in_seconds, label="Unoptimised")
plt.xlabel("n")
plt.ylabel("Time (s)")
plt.title("Factorial Benchmark Results (Linear Scale)")
plt.legend()
plt.grid(True)
plt.savefig("factorial_benchmark_linear.png")  
plt.close()

plt.plot(n_values, mean_estimates_opt_in_seconds, label="Optimised")
plt.plot(n_values, mean_estimates_unop_in_seconds, label="Unoptimised")
plt.xlabel("n")
plt.ylabel("Time (s)")
plt.title("Factorial Benchmark Results (Log Scale)")
plt.legend()
plt.grid(True)
plt.yscale('log')
plt.savefig("factorial_benchmark_log.png")  

