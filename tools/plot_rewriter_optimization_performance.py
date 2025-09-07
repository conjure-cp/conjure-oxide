import json
import matplotlib.pyplot as plt


def load_data_from_file(file_path):
    with open(file_path, "r") as file:
        return json.load(file)


def plot_performance_gain(data):
    x = [key for key in sorted(data.keys())]
    y = [data[key]["performance_gain"] for key in sorted(data.keys())]

    plt.figure(figsize=(8, 6))
    plt.plot(x, y, marker="o")
    plt.xlabel("Number of OR Clauses")
    plt.ylabel("Performance Gain (%)")
    plt.title("Performance Gain between Optimized and Unoptimized Versions")
    plt.grid(True)
    plt.savefig("performance_gain.png")
    plt.close()


def plot_rule_application_savings(data):
    x = [key for key in sorted(data.keys())]
    y = [data[key]["rule_application_savings"] for key in sorted(data.keys())]

    plt.figure(figsize=(8, 6))
    plt.plot(x, y, marker="o", color="g")
    plt.xlabel("Number of OR Clauses")
    plt.ylabel("Rule Application Savings (%)")
    plt.title("Rule Application Savings between Optimized and Unoptimized Versions")
    plt.grid(True)
    plt.savefig("rule_application_savings.png")
    plt.close()


def main():
    output_files = [
        "rewrite_solve_xyz_optimized_1-stats.json",
        "rewrite_solve_xyz_optimized_2-stats.json",
        "rewrite_solve_xyz_optimized_3-stats.json",
        "rewrite_solve_xyz_optimized_4-stats.json",
        "rewrite_solve_xyz_unoptimized_1-stats.json",
        "rewrite_solve_xyz_unoptimized_2-stats.json",
        "rewrite_solve_xyz_unoptimized_3-stats.json",
        "rewrite_solve_xyz_unoptimized_4-stats.json",
    ]

    results = {}
    optimized_performance = []
    unoptimized_performance = []
    optimized_rule_applications = []
    unoptimized_rule_applications = []

    for file_name in output_files:
        with open(file_name, "r") as file:
            data = json.load(file)
            if "unoptimized" in file_name:
                unoptimized_performance.append(
                    data["stats"]["rewriterRuns"][0]["rewriterRunTime"]["nanos"]
                    + data["stats"]["rewriterRuns"][0]["rewriterRunTime"]["secs"] * 1e9
                )
                unoptimized_rule_applications.append(
                    data["stats"]["rewriterRuns"][0]["rewriterRuleApplicationAttempts"]
                )
            else:
                optimized_performance.append(
                    data["stats"]["rewriterRuns"][0]["rewriterRunTime"]["nanos"]
                    + data["stats"]["rewriterRuns"][0]["rewriterRunTime"]["secs"] * 1e9
                )
                optimized_rule_applications.append(
                    data["stats"]["rewriterRuns"][0]["rewriterRuleApplicationAttempts"]
                )

    for i in range(4):
        performance_gain = (
            (unoptimized_performance[i] - optimized_performance[i])
            / optimized_performance[i]
            * 100
        )
        rule_application_savings = (
            (unoptimized_rule_applications[i] - optimized_rule_applications[i])
            / unoptimized_rule_applications[i]
            * 100
        )
        results[i + 1] = {
            "performance_gain": performance_gain,
            "rule_application_savings": rule_application_savings,
        }

    plot_performance_gain(results)
    plot_rule_application_savings(results)


if __name__ == "__main__":
    main()
