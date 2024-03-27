# conjure_oxide_benchmarks.py
# author: Pedro Gronda Garrigues

# dependencies
import os
import subprocess

# define path to conjure
conjure_path = "..."

# function to run conjure on test files
def process_essence_files(directory):
    for root, dirs, files in os.walk(directory):
        for file in files:
            if file.endswith(".essence"):
                # construct full file path
                essence_file_path = os.path.join(root, file)
                # construct command
                command = f"conjure "