# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Progress(Component):
    """A Progress component.


Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component. Use this to nest progress bars.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- animated (boolean; optional):
    Animate the bar, must have striped set to True to work.

- bar (boolean; optional):
    Set to True when nesting Progress inside another Progress
    component to create a multi-progress bar.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- color (string; optional):
    Set color of the progress bar, options: primary, secondary,
    success, warning, danger, info or any valid CSS color of your
    choice (e.g. a hex code, a decimal code or a CSS color name).

- hide_label (boolean; default False):
    Set to True to hide the label.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- label (string; optional):
    Adds a label to the progress bar.

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

- max (number; optional):
    Upper limit for value, default: 100.

- min (number; optional):
    Lower limit for value, default: 0.

- striped (boolean; optional):
    Use striped progress bar.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- value (string | number; optional):
    Specify progress, value from min to max inclusive."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Progress'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, bar=Component.UNDEFINED, min=Component.UNDEFINED, max=Component.UNDEFINED, value=Component.UNDEFINED, label=Component.UNDEFINED, hide_label=Component.UNDEFINED, animated=Component.UNDEFINED, striped=Component.UNDEFINED, color=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'animated', 'bar', 'className', 'class_name', 'color', 'hide_label', 'key', 'label', 'loading_state', 'max', 'min', 'striped', 'style', 'value']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'animated', 'bar', 'className', 'class_name', 'color', 'hide_label', 'key', 'label', 'loading_state', 'max', 'min', 'striped', 'style', 'value']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Progress, self).__init__(children=children, **args)
