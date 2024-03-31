# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Placeholder(Component):
    """A Placeholder component.
Use loading Placeholders for your components or pages to indicate
something may still be loading.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- animation (a value equal to: 'glow', 'wave'; optional):
    Changes the animation of the placeholder.

- button (boolean; default False):
    Show as a button shape.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- color (string; optional):
    Background color, options: primary, secondary, success, info,
    warning, danger, light, dark.

- delay_hide (number; default 0):
    When using the placeholder as a loading placeholder, add a time
    delay (in ms) to the placeholder being removed to prevent
    flickering.

- delay_show (number; default 0):
    When using the placeholder as a loading placeholder, add a time
    delay (in ms) to the placeholder being shown after the
    loading_state is set to True.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- lg (number; optional):
    Specify placeholder behaviour on a large screen.  Valid arguments
    are boolean, an integer in the range 1-12 inclusive. See the
    documentation for more details.

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

- md (number; optional):
    Specify placeholder behaviour on a medium screen.  Valid arguments
    are boolean, an integer in the range 1-12 inclusive. See the
    documentation for more details.

- show_initially (boolean; default True):
    Whether the Placeholder should show on app start-up before the
    loading state has been determined. Default True.

- size (a value equal to: 'xs', 'sm', 'lg'; optional):
    Component size variations. Only valid when `button=False`.

- sm (number; optional):
    Specify placeholder behaviour on a small screen.  Valid arguments
    are boolean, an integer in the range 1-12 inclusive. See the
    documentation for more details.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- xl (number; optional):
    Specify placeholder behaviour on an extra large screen.  Valid
    arguments are boolean, an integer in the range 1-12 inclusive. See
    the documentation for more details.

- xs (number; optional):
    Specify placeholder behaviour on an extra small screen.  Valid
    arguments are boolean, an integer in the range 1-12 inclusive. See
    the documentation for more details.

- xxl (number; optional):
    Specify placeholder behaviour on an extra extra large screen.
    Valid arguments are boolean, an integer in the range 1-12
    inclusive. See the documentation for more details."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Placeholder'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, loading_state=Component.UNDEFINED, animation=Component.UNDEFINED, color=Component.UNDEFINED, size=Component.UNDEFINED, button=Component.UNDEFINED, delay_hide=Component.UNDEFINED, delay_show=Component.UNDEFINED, show_initially=Component.UNDEFINED, xs=Component.UNDEFINED, sm=Component.UNDEFINED, md=Component.UNDEFINED, lg=Component.UNDEFINED, xl=Component.UNDEFINED, xxl=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'animation', 'button', 'className', 'class_name', 'color', 'delay_hide', 'delay_show', 'key', 'lg', 'loading_state', 'md', 'show_initially', 'size', 'sm', 'style', 'xl', 'xs', 'xxl']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'animation', 'button', 'className', 'class_name', 'color', 'delay_hide', 'delay_show', 'key', 'lg', 'loading_state', 'md', 'show_initially', 'size', 'sm', 'style', 'xl', 'xs', 'xxl']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Placeholder, self).__init__(children=children, **args)
