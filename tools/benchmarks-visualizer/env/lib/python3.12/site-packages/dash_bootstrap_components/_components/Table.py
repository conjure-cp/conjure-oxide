# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Table(Component):
    """A Table component.
A component for applying Bootstrap styles to HTML tables. Use this as a
drop-in replacement for `html.Table`, or generate a table from a Pandas
DataFrame using `dbc.Table.from_dataframe`.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- bordered (boolean; optional):
    Apply the `table-bordered` class which adds borders on all sides
    of the table and cells.

- borderless (boolean; optional):
    Apply the `table-borderless` class which removes all borders from
    the table and cells.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- color (string; optional):
    Table color, options: primary, secondary, success, info, warning,
    danger, dark, light. Default: secondary.

- dark (boolean; optional):
    **DEPRECATED** - Use color=\"dark\" instead.  Apply the
    `table-dark` class for dark cell backgrounds and light text.

- hover (boolean; optional):
    Apply the `table-hover` class which enables a hover state on table
    rows within the table body.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- loading_state (dict; optional):
    Object that holds the loading state object coming from
    dash-renderer.

    `loading_state` is a dict with keys:

    - component_name (string; optional):
        Holds the name of the component that is loading.

    - is_loading (boolean; optional):
        Determines if the component is loading or not.

    - prop_name (string; optional):
        Holds which property is loading.

- responsive (boolean | string; optional):
    Set to True or one of the breakpoints 'sm', 'md', 'lg', 'xl' to
    make table scroll horizontally at lower breakpoints.

- size (string; optional):
    Specify table size, options: 'sm', 'md', 'lg'.

- striped (boolean; optional):
    Apply the `table-striped` class which applies 'zebra striping' to
    rows in the table body.

- style (dict; optional):
    Defines CSS styles which will override styles previously set."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Table'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, size=Component.UNDEFINED, bordered=Component.UNDEFINED, borderless=Component.UNDEFINED, striped=Component.UNDEFINED, color=Component.UNDEFINED, dark=Component.UNDEFINED, hover=Component.UNDEFINED, responsive=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'bordered', 'borderless', 'className', 'class_name', 'color', 'dark', 'hover', 'key', 'loading_state', 'responsive', 'size', 'striped', 'style']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'bordered', 'borderless', 'className', 'class_name', 'color', 'dark', 'hover', 'key', 'loading_state', 'responsive', 'size', 'striped', 'style']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Table, self).__init__(children=children, **args)
