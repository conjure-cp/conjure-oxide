# @script: generate_qmd_file.py
# @author: Pedro Gronda Garrigues

# dependencies
import os
import sys

# set the directory where figures stored
figures_dir = './figures'

# function to list down all image files in the figures directory
def list_figure_files(directory):
    # [SANITY CHECK] checks to see if the directory for node & time elapsed figures is empty
    if not os.listdir(directory):
        sys.exit("FATAL ERROR: The './figures' directory is empty. Please generate the appropriate solver statistics.")
    return [f for f in os.listdir(directory)]

# function to write the content to the .qmd file
def write_quarto_file(figure_files):
    with open('html/dashboard.qmd', 'w') as qmd_file:
        # write the front matter with title, author, and output format
        qmd_file.write('---\n')
        qmd_file.write('title: "Benchmarks Visualizer: Native vs Oxide"\n')
        qmd_file.write('author: "Pedro Gronda Garrigues"\n')
        qmd_file.write('format: html\n')
        qmd_file.write('---\n\n')

        # extract categories and test names
        categories = set(file.split('_')[1] for file in figure_files)
        tests = set((file.split('_')[1], file.partition('_')[2].partition('_')[2].rsplit('.png')[0]) for file in figure_files)

        # add images under their respective category and test folder
        for category in sorted(categories):
            qmd_file.write(f'# TEST CATEGORY: {category}\n\n')
            for test in sorted(tests):
                if test[0] == category:
                    test_name = test[1]

                    qmd_file.write(f'### Test Folder: {test_name}\n\n')

                    nodes_graph = f"nodes_{category}_{test_name}.png"
                    time_graph = f"time_{category}_{test_name}.png"


                    qmd_file.write(f'<div>\n')
                    qmd_file.write(f'<img src=".{os.path.join(figures_dir, nodes_graph)}" alt="Solver Nodes for {category} {test_name}" style="display:inline-block; width:49%; margin-right:1%;" />\n')
                    qmd_file.write(f'<img src=".{os.path.join(figures_dir, time_graph)}" alt="Time Elapsed (ms) for {category} {test_name}" style="display:inline-block; width:49%;" />\n')
                    qmd_file.write(f'</div>\n\n')

# get all figure files from the figures directory
figure_files = list_figure_files(figures_dir)

# write the quarto markdown file
write_quarto_file(figure_files)

print(f"STATUS: Quarto markdown file 'dashboard.qmd' has been created. Total tests: {len(figure_files)}")
