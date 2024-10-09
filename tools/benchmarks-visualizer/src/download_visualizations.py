# @script: download_visualizations.py
# @author: Pedro Gronda Garrigues

# dependencies
from processing import extract_solver_stats_dataframe

# plotting libraries
import plotly.graph_objects as go
import plotly.io as pio
import os

# extract data
root_dir = './data'
df = extract_solver_stats_dataframe(root_dir)

# set directory for saving figures
figures_dir = './figures' # in benchmarks-visualizer dir

# function to save figure as image
def save_figure(figure, filename):
    file_path = f"{figures_dir}/{filename}.png"

    try:
        pio.write_image(figure, file_path)
        # print(f"STATUS: Figure saved as {file_path}") # output to help verify successful save
    except Exception as e:
        print(f"ERROR: Unable to save figure as {file_path}: {e}")

# static generation of visualizations
def generate_static_visualizations(df):
    categories = df.index.get_level_values('Test Category').unique()
    
    for category in categories:
        test_folders = df.loc[category].index.get_level_values('Test').unique()
        

        for test_folder in test_folders:

            filtered_df = df.loc[(category, test_folder)]
            # print(filtered_df)

            condition = filtered_df['Status'] == 'OK'

            ########### SOLVER NODE BAR GRAPH ###########

            # get unique solver names (as a precaution)
            solvers = filtered_df.index.get_level_values('Solver').unique()
            solver_pairs = []
            #  find matching solvers (modular for added solvers)
            ## for more optimized code, hard code the solvers currently in use
            for solver in solvers:
                if solver.startswith('native_'):
                    counterpart = 'oxide_' + solver.split('_', 1)[1]
                elif solver.startswith('oxide_'):
                    counterpart = 'native_' + solver.split('_', 1)[1]
                else:
                    continue

                if counterpart in solvers:
                    # append pairs and remove duplicates
                    pair = tuple(sorted([solver, counterpart]))
                    if pair not in solver_pairs:
                        solver_pairs.append(pair)

            bar_data = []
            used_solvers = set()
            for solver_pair in solver_pairs:
                for solver in solver_pair:
                    bar_data.append(go.Bar(
                        name=solver,
                        x=[solver.replace('native_', '').replace('oxide_', '')],
                        y=[filtered_df.loc[solver]['SolverNodes']],
                        width=0.4,
                        offset=-0.2 if 'native_' in solver else 0.2,  # Adjust position based on solver type
                    ))
                    used_solvers.add(solver)

            # add rest of solvers
            for solver in solvers:
                if solver not in used_solvers:
                    bar_data.append(go.Bar(
                        name=solver,
                        x=[solver.replace('native_', '').replace('oxide_', '')],
                        y=[filtered_df.loc[solver]['SolverNodes']],
                        width=0.4,
                        offset=0
                    ))

            # create the figure using bar data
            nodes_fig = go.Figure(data=bar_data)
            nodes_fig.update_layout(
                title=f'Solver Nodes for {test_folder}',
                xaxis_title='Solver',
                yaxis_title='Nodes',
                barmode='group'
            )
            save_figure(nodes_fig, f'nodes_{category}_{test_folder}')
            
            ########### SOLVER TIME ELAPSED GRAPH ###########

            time_fig = go.Figure(data=[
                go.Bar(
                    x=filtered_df[condition].index.get_level_values('Solver'),
                    y=filtered_df[condition]['TotalTime']
                )
            ])
            time_fig.update_layout(title=f'Total Time (Elapsed) for {test_folder}', xaxis_title='Solver', yaxis_title='Time (ms)')
            save_figure(time_fig, f'time_{category}_{test_folder.rstrip('.png')}') # stripping .png just in case

if __name__ == '__main__':
    generate_static_visualizations(df)
