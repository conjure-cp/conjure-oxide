# calculate_coverage_difference.py
# @author Pedro Gronda Garrigues

import re
import sys

def parse_lcov(filepath):
    """
    Opens provided filepath file and uses regex to find lines LF and LH

    :param filepath: path to the lcov file
    :return covered_lines: number of lines covered
    :return total_lines: total number of lines
    """

    with open(filepath, 'r') as file:
        lcov_data = file.read()
        # use regex to find lines of the form: "LF:<number of instrumented lines>"
        total_lines = sum(map(int, re.findall(r'^LF:(\d+)', lcov_data, flags=re.MULTILINE)))
        # use regex to find lines of the form: "LH:<number of hit lines>"
        covered_lines = sum(map(int, re.findall(r'^LH:(\d+)', lcov_data, flags=re.MULTILINE)))

    return covered_lines, total_lines

def calculate_percentage(covered_lines, total_lines):
    return (covered_lines / total_lines) * 100 if total_lines > 0 else 0

def compare_coverages(base_filepath, new_filepath):
    base_covered, base_total = parse_lcov(base_filepath)
    new_covered, new_total = parse_lcov(new_filepath)

    base_percentage = calculate_percentage(base_covered, base_total)
    new_percentage = calculate_percentage(new_covered, new_total)

    diff_percentage = new_percentage - base_percentage
    return base_percentage, new_percentage, diff_percentage

if __name__ == "__main__":
    # sanity check for both files provided
    if len(sys.argv) != 3:
        print("Usage: python calculate_coverage_difference.py <base_lcov_info> <new_lcov_info>")
        exit(1)
    
    base_lcov = sys.argv[1]
    new_lcov = sys.argv[2]

    # calculate coverage differences
    base_percent, new_percent, diff = compare_coverages(base_lcov, new_lcov)
    
    # output the results to be cat into code coverage summary
    print(f"Previous Line Coverage: {base_percent:.2f}%")
    print(f"New Line Coverage: {new_percent:.2f}%")
    sign = "+" if diff >= 0 else "-"
    print(f"Change in Coverage: {sign}{abs(diff):.2f}%")
