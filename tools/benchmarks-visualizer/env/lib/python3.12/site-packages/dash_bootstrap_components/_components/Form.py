# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Form(Component):
    """A Form component.
The Form component can be used to organise collections of input components
and apply consistent styling.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- action (string; optional):
    The URI of a program that processes the information submitted via
    the form.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

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

- method (a value equal to: 'GET', 'POST'; optional):
    Defines which HTTP method to use when submitting the form. Can be
    GET (default) or POST.

- n_submit (number; default 0):
    Number of times the `Enter` key was pressed while the input had
    focus.

- n_submit_timestamp (number; default -1):
    Last time that `Enter` was pressed.

- prevent_default_on_submit (boolean; default True):
    The form calls preventDefault on submit events. If you want form
    data to be posted to the endpoint specified by `action` on submit
    events, set prevent_default_on_submit to False. Defaults to True.

- style (dict; optional):
    Defines CSS styles which will override styles previously set."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Form'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, action=Component.UNDEFINED, method=Component.UNDEFINED, n_submit=Component.UNDEFINED, n_submit_timestamp=Component.UNDEFINED, prevent_default_on_submit=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'action', 'className', 'class_name', 'key', 'loading_state', 'method', 'n_submit', 'n_submit_timestamp', 'prevent_default_on_submit', 'style']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'action', 'className', 'class_name', 'key', 'loading_state', 'method', 'n_submit', 'n_submit_timestamp', 'prevent_default_on_submit', 'style']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Form, self).__init__(children=children, **args)
