import styles as page
import processing as processing
import plotly_express as px
from dash import Dash, dcc, html, callback, Output, Input
import plotly.graph_objects as go

boxplotdata = processing.generateBoxplotdata(processing.data)
scatterplotdata = processing.generateScatterplotdata(processing.data)
problemNames = processing.data["Problem"].unique()
#scatterplot = processing.createScatterplot(scatterplotdata)

resultants = []
indices = []
# paragraphs = [html.H3("Anomalous cases to investigate")]
# with open('./anomalies.txt', 'r') as f:
#     lines = f.readlines()
#     for line in lines:
#         paragraphs.append(html.P(children=line))
# we create all the elements and put them in a list
# then we simply set the children attribute of the app.layout div to be elements
# this allows us to construct elements bit by bit, without making app layout wildly messy
elements = []

# we first add a header
elements.append(html.Section(children=(html.H1(children="Solver Analytics", style=page.topsectionh1style, ),
                                       html.P(
                                           children=page.topsectionh1text,
                                           style=page.introStyle),),
                             style=page.topsectionstyle, ))


processing.generateScatterplot(elements, scatterplotdata)
# dropdowns for specifying the box plot graph
elements.append(dcc.Dropdown(boxplotdata["Problem"].unique(), 'csplib-prob001', id='problemDropdown'))
elements.append(dcc.Dropdown(boxplotdata["Model"].unique(), 'model.eprime', id='modelDropdown'))
elements.append(dcc.Dropdown(boxplotdata["Solver"].unique(), 'lingeling', id='solverDropdown'))

elements.append(dcc.Graph(id="boxPlot", style=page.barChartStyle))
#elements.append(html.Section(children=paragraphs))
# a footer section
elements.append(html.Section(
    children=[html.P(["A Vertically Integrated Project by Samvit Nagpal, University of St Andrews, 2024", html.Br(),
                      "Under the supervision of Ozgur Akgun"], style=page.footerTextStyle)], style=page.footerStyle))





app = Dash(meta_tags=[
    {"name": "viewport", "content": "width=device-width, initial-scale=1"}
])

app.layout = html.Div(
    children=elements, style=page.body,
)

# callback for selecting problem, solver and parameters to be displayed on the box plot
@callback(Output('boxPlot', 'figure'),
          [Input('problemDropdown', 'value')], [Input('modelDropdown', 'value')],
          [Input('solverDropdown', 'value')])
def update_boxplot(problem, model, solver):

    return px.box(boxplotdata,
                  y=boxplotdata.loc[
                      ((boxplotdata["Problem"] == problem) & (boxplotdata["Model"] == model) & (boxplotdata["Solver"] == solver))][
                      "Total Solution Time"],
                  title=("Solving " + problem + " with model " + model + " and solver " + solver), labels={

            "y": "Solution Time (ms)"
        })

@callback(Output('scatterplot', 'figure'),
          [Input('problemDropdown', 'value')], [Input('scattermodelDropdown1', 'value')],
          [Input('scattersolverDropdown1', 'value')], [Input('scattermodelDropdown2', 'value')],
          [Input('scattersolverDropdown2', 'value')], [Input('scattertimeDropdown', 'value')])
def update_Scatterplot(problem, model1, solver1, model2, solver2, time):
    scatterplot = px.scatter(x=scatterplotdata.loc[(scatterplotdata["Problem"] == problem) & (scatterplotdata["Solver"] == solver1) & (scatterplotdata["Model"] == model1) & (scatterplotdata["Options"] == time)]["Time"],
                             y=scatterplotdata.loc[(scatterplotdata["Problem"] == problem) & (scatterplotdata["Solver"] == solver2) & (scatterplotdata["Model"] == model2) & (scatterplotdata["Options"] == time)]["Time"],
                             labels={"x": "Time taken by " + solver1, "y": "Time taken by " + solver2})
    scatterplot.add_trace(
        go.Scatter(x=scatterplotdata.loc[scatterplotdata["Solver"] == "minion"]["Time"],
                   y=scatterplotdata.loc[scatterplotdata["Solver"] == "minion"]["Time"], name="y = x")
    )
    # changing the axis ranges and adding a bit of styling
    scatterplot.update_xaxes(
        range=[0, max(scatterplotdata.loc[(scatterplotdata["Problem"] == problem) & (scatterplotdata["Solver"] == solver1) & (scatterplotdata["Model"] == model1) & (scatterplotdata["Options"] == time)]["Time"])],
        showline=True, linewidth=1, linecolor='black'
    )

    scatterplot.update_yaxes(
        range=[0, max(scatterplotdata.loc[(scatterplotdata["Problem"] == problem) & (scatterplotdata["Solver"] == solver2) & (scatterplotdata["Model"] == model2) & (scatterplotdata["Options"] == time)]["Time"])],
        showline=True, linewidth=1, linecolor='black'

    )
    return scatterplot

app.run_server(debug=True)