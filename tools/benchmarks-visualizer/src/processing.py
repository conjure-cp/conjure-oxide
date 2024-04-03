# @script: processing.py, to extract information from JSON files
# @author: Pedro Gronda Garrigues

# dependencies
import json
import os
from glob import glob
import pandas as pd

# function to extract information and return a DataFrame
def extract_solver_stats_dataframe(root_dir):
    rows = []

    # iterate through all exhaustive test category (i.e. autogen, basic, etc)
    for test_category in os.listdir(root_dir):
        test_category_path = os.path.join(root_dir, test_category)

        if os.path.isdir(test_category_path):
            for test_dir in os.listdir(test_category_path):
                test_path = os.path.join(test_category_path, test_dir)

                print(f'Processing {test_path}')
                if os.path.isdir(test_path):
                    # conjure oxide minion solver json parse
                    oxide_minion_file = glob(os.path.join(test_path, '*oxide_minion*.json'))
                    if oxide_minion_file:
                        try:
                            with open(oxide_minion_file[0], 'r') as f:
                                data = json.load(f)
                                
                                # Assuming there's only one solverRun in the list.
                                run_info = data.get('stats', {}).get('solverRuns', [])[0]

                                if run_info and run_info.get('solverAdaptor', '') == 'Minion':
                                    essence = os.path.basename(data['fileName'])
                                    nodes = float(run_info.get('nodes', 0))
                                    # NOTE: conjureSolverWallTime_s total time is in seconds, convert to milliseconds
                                    total_time = run_info.get('conjureSolverWallTime_s', 0) * 100
                                    
                                    rows.append({
                                        "Test Category": test_category,
                                        "Test": test_dir,
                                        "Solver": 'oxide_minion',
                                        "Essence": essence,
                                        "SolverNodes": nodes,
                                        "Status": 'OK',
                                        "TotalTime": total_time
                                    })

                        except json.JSONDecodeError as e:
                            print(f"Error decoding JSON from file {oxide_minion_file}: {e}")
                        except FileNotFoundError as e:
                            print(f"File not found {oxide_minion_file}: {e}")
                        except Exception as e:  # Handle other exceptions such as permission issues
                            print(f"An unexpected error occurred processing the oxide_minion file: {e}")

                    # conjure native solver json parse
                    for solver in ['chuffed', 'glucose', 'glucose-syrup', 'kissat', 'lingeling', 'minion']:
                        stats_files = glob(os.path.join(test_path, solver, '*.stats.json'))
                        # print(f"\tStat file: {stats_files}")

                        # a solver test can have more than one stats file depending on the parameter files
                        for stats_file in stats_files:
                            try:
                                with open(stats_file, 'r') as f:
                                    data = json.load(f)

                                    essence = os.path.basename(data['essence'])
                                    nodes = float(data.get('savilerowInfo', {}).get('SolverNodes', 0))
                                    status = data.get('status', 'Invalid').upper()
                                    total_time = data.get('totalTime', 0)
                        
                                    rows.append({
                                        "Test Category": test_category,
                                        "Test": test_dir,
                                        "Solver": "native_{}".format(solver),
                                        "Essence": essence,
                                        "SolverNodes": nodes,
                                        "Status": status,
                                        "TotalTime": total_time
                                    })
                            except json.JSONDecodeError as e:
                                print(f"Error decoding JSON from file {stats_file}: {e}")
                            except FileNotFoundError as e:
                                print(f"File not found {stats_file}: {e}")
                            except Exception as e:  # Handle other exceptions such as permission issues
                                print(f"An unexpected error occurred: {e}")

    # convert the list of dictionaries to a DataFrame             
    df = pd.DataFrame(rows)

    try:
        # set up a MultiIndex using the 'Test Category', 'Test', and 'Solver' columns
        df.set_index(['Test Category', 'Test', 'Solver'], inplace=True)
    except KeyError as e:
        # KeyError here indicates that one of the columns doesn't exist in the dataframe (incomplete scraping of data)
        print(f"Error: {e}. The data is not complete for conjure oxide and native.")
        print("Please execute the data scripts to ensure all required data is present.")
    
    # sort the index to make it easier to slice and view
    df.sort_index(inplace=True)

    # return DataFrame
    return df

if __name__ == '__main__':
    root_dir = './data'  # Replace with your actual data folder path
    
    print(extract_solver_stats_dataframe(root_dir))

