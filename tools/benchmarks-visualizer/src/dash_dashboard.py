# @script: app.py, to run dashboard using Dash
# @author: Pedro Gronda Garrigues

# dependencies
from processing import extract_solver_stats_dataframe  # Updated function name

# import dash application libraries
from dash import Dash, dcc, html, Input, Output
from dash.exceptions import PreventUpdate
import plotly.graph_objs as go
import dash_bootstrap_components as dbc

root_dir = './data'                           # replace with actual data folder path
df = extract_solver_stats_dataframe(root_dir) # ppdated function call

# initiate the Dash app
app = Dash(__name__, external_stylesheets=[dbc.themes.BOOTSTRAP])

# assuming Test Category Dropdown will dictate what tests show up in Test Folder Dropdown
app.layout = html.Div([
    # title and author Tag
    html.H1("Benchmarks Visualizer: Native vs Oxide", style={'textAlign': 'center', 'marginTop': '10px'}),
    html.Div(
        "Author: Pedro Gronda Garrigues",
        style={'textAlign': 'center', 'marginBottom': '10px'}
    ),

    # dropdown to select Test Category
    dcc.Dropdown(
        id='test-category-dropdown',
        options=[{'label': i, 'value': i} for i in df.index.get_level_values('Test Category').unique()],
        value=df.index.get_level_values('Test Category').unique()[0]
    ),
    # dropdown to select Test Folder based on Test Category
    dcc.Dropdown(
        id='test-folder-dropdown',  # options set by callback
    ),
    dcc.Graph(id='solver-nodes-graph'),
    dcc.Graph(id='total-time-graph')
])

# app callback (asynchronous function) to update Test Folder Dropdown based on Test Category Dropdown
@app.callback(
    Output('test-folder-dropdown', 'options'),
    [Input('test-category-dropdown', 'value')]
)

# function to set Test Folder Dropdown options based on selected Test Category
def set_test_folder_options(selected_test_category):
    test_folders = df.loc[selected_test_category].index.get_level_values('Test').unique()
    return [{'label': i, 'value': i} for i in test_folders]

@app.callback(
    [Output('solver-nodes-graph', 'figure'),
     Output('total-time-graph', 'figure')],
    [Input('test-folder-dropdown', 'value'),
     Input('test-category-dropdown', 'value')]
)

def update_graphs(selected_test, selected_category):
    if selected_test is None:
        raise PreventUpdate
    
    filtered_df = df.loc[(selected_category, selected_test)]
    
    # assuming Solver Status OK data is desired
    condition = filtered_df['Status'] == 'OK'
    
    # nodes Bar Graph
    ### *NOTE: comparing nodes for SavileRow accross conjure native solvers is irrelevant
    ###        it is, however, useful to compare the for Native vs Oxide.
    ###        Seeing as there is only support for Oxide Minion so far (April 2024), the nodes are still on
    ###        display for all solvers. The end goal is to have Native vs Oxide solvers as discrete variables
    ###        to directly compare the performance of the two.
    nodes_fig = go.Figure(data=[
        go.Bar(
            x=filtered_df[condition].index.get_level_values('Solver'),
            y=filtered_df[condition]['SolverNodes'],
            text=filtered_df[condition]['SolverNodes']
        )
    ])
    nodes_fig.update_layout(title='Solver Nodes', xaxis_title='Solver', yaxis_title='Nodes')
    
    # total Time Bar Graph
    time_fig = go.Figure(data=[
        go.Bar(
            x=filtered_df[condition].index.get_level_values('Solver'),
            y=filtered_df[condition]['TotalTime'],
            text=filtered_df[condition]['TotalTime']
        )
    ])
    time_fig.update_layout(title='Total Time (Elapsed)', xaxis_title='Solver', yaxis_title='Time (ms)')
    
    return nodes_fig, time_fig

# run the app
if __name__ == '__main__':
    app.run_server(debug=True)
