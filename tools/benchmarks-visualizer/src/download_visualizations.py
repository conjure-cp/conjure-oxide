# @script: download_visualizations.py
# @author: Pedro Gronda Garrigues

# dependencies
from processing import extract_solver_stats_dataframe

# plotting libraries
import plotly.graph_objs as go
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
        print(f"STATUS: Figure saved as {file_path}") # output to help verify successful save
    except Exception as e:
        print(f"ERROR: Unable to save figure as {file_path}: {e}")

# static generation of visualizations
def generate_static_visualizations(df):
    categories = df.index.get_level_values('Test Category').unique()
    
    for category in categories:
        test_folders = df.loc[category].index.get_level_values('Test').unique()
        
        for test_folder in test_folders:

            filtered_df = df.loc[(category, test_folder)]
            condition = filtered_df['Status'] == 'OK'

            # solver Nodes Bar Graph
            nodes_fig = go.Figure(data=[
                go.Bar(
                    x=filtered_df[condition].index.get_level_values('Solver'),
                    y=filtered_df[condition]['SolverNodes']
                )
            ])
            nodes_fig.update_layout(title=f'Solver Nodes for {test_folder}', xaxis_title='Solver', yaxis_title='Nodes')
            save_figure(nodes_fig, f'nodes_{category}_{test_folder}')
            
            # total Time Bar Graph
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
