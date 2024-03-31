# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Fade(Component):
    """A Fade component.
Hide or show content with a fading animation. Visibility of the children is
controlled by the `is_open` prop which can be targetted by callbacks.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- appear (boolean; optional):
    Show fade-in animation on initial page load. Default: True.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- enter (boolean; optional):
    Enable or disable enter transitions. Default: True.

- exit (boolean; optional):
    Enable or disable exit transitions. Default: True.

- is_in (boolean; optional):
    Controls whether the children of the Fade component are currently
    visible or not.

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

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- tag (string; optional):
    HTML tag to use for the fade component. Default: div.

- timeout (dict; optional):
    The duration of the transition, in milliseconds.  You may specify
    a single timeout for all transitions like: `timeout=500` or
    individually like: timeout={'enter': 300, 'exit': 500}.

    `timeout` is a number | dict with keys:

    - enter (number; optional)

    - exit (number; optional)"""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Fade'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, is_in=Component.UNDEFINED, timeout=Component.UNDEFINED, appear=Component.UNDEFINED, enter=Component.UNDEFINED, exit=Component.UNDEFINED, tag=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'appear', 'className', 'class_name', 'enter', 'exit', 'is_in', 'key', 'loading_state', 'style', 'tag', 'timeout']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'appear', 'className', 'class_name', 'enter', 'exit', 'is_in', 'key', 'loading_state', 'style', 'tag', 'timeout']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Fade, self).__init__(children=children, **args)
