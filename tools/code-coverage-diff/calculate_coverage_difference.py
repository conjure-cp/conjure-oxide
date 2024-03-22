# calculate_coverage_difference.py
# @author Pedro Gronda Garrigues

# dependencies
import sys
import re

def parse_coverage_data(coverage_data):
    """
    Parse the coverage data from the raw input format to a dictionary.

    :param coverage_data: string containing the coverage information.
    :return: dictionary with coverage categories as keys and tuples of percentages and counts as values.
             Returns None as value if no data was found for a category.
    """
    
    coverage_dict = {}
    for line in coverage_data.splitlines():
        match = re.match(r'(\w+).*: ([\d.]+)% \((\d+) of (\d+)', line)
        if match:
            # store coverage details in a dictionary
            category = match.group(1).lower()
            percentage = float(match.group(2))
            covered = int(match.group(3))
            total = int(match.group(4))
            coverage_dict[category] = (percentage, covered, total)
        elif 'no data found' in line:
            # handle 'no data found' case by setting to None
            category = line.split(':')[0].strip().lower()
            coverage_dict[category] = None
    
    return coverage_dict

def calculate_differences(main_data, pr_data):
    """
    Calculates the differences in coverage between the main branch and a pull request.

    :param main_data: coverage data from the main branch.
    :param pr_data: coverage data from the pull request.
    :return: a dictionary with the coverage difference for each category.
    """

    # parses both inputs into dictionaries
    main_cov = parse_coverage_data(main_data)
    pr_cov = parse_coverage_data(pr_data)
    diffs = {}
    
    # iterates through each category to calculate differences
    for category in main_cov.keys():
        if main_cov[category] is None or pr_cov[category] is None:
            # no comparison data available for this category
            diffs[category] = {'diff_percentage': "no data"}
        else:
            # calculate difference in coverage percentage and covered units
            diff_percentage = pr_cov[category][0] - main_cov[category][0]
            diff_covered = pr_cov[category][1] - main_cov[category][1]
            diffs[category] = {
                'diff_percentage': f"{diff_percentage:.2f}%",
                'diff_covered': diff_covered
            }
            
    return diffs

def format_diff_output(diffs):
    """
    Formats the differences in coverage data into a readable string output.

    :param diffs: a dictionary containing the coverage differences.
    :return: formatted string with the changes in coverage data.
    """

    lines = []
    # construct the output for each coverage category
    for category, diff_data in diffs.items():
        if diff_data['diff_percentage'] == "no data":
            lines.append(f"{category.capitalize()} coverage: No comparison data available")
        else:
            lines.append(f"{category.capitalize()} coverage changed by {diff_data['diff_percentage']} and covered lines changed by {diff_data['diff_covered']}")
    
    return "\n".join(lines)

def main():
    try:
        # get parameter data for main and pr (or historical main and main)
        main_coverage_data = sys.argv[1]
        pr_coverage_data = sys.argv[2]

        # calculate the differences between the sets of data
        diffs = calculate_differences(main_coverage_data, pr_coverage_data)
        
        # format output using differences
        formatted_output = format_diff_output(diffs)
        
        # print for bash stdout capture
        print(formatted_output)
    except IndexError:
        print("ERROR: Missing required command-line arguments.")
    except Exception as e:
        # generic error handling
        print(f"An error occurred: {e}")

if __name__ == "__main__":
    main()
