# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Spinner(Component):
    """A Spinner component.
Render Bootstrap style loading spinners using only CSS.

This component can be used standalone to render a loading spinner, or it can
be used like `dash_core_components.Loading` by giving it children. In the
latter case the chosen spinner will display while the children are loading.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- color (string; optional):
    Sets the color of the Spinner. Main options are Bootstrap
    contextual colors: primary, secondary, success, info, warning,
    danger, light, dark, body, muted, white-50, black-50. You can also
    specify any valid CSS color of your choice (e.g. a hex code, a
    decimal code or a CSS color name)  If not specified will default
    to text colour.

- delay_hide (number; default 0):
    When using the spinner as a loading spinner, add a time delay (in
    ms) to the spinner being removed to prevent flickering.

- delay_show (number; default 0):
    When using the spinner as a loading spinner, add a time delay (in
    ms) to the spinner being shown after the loading_state is set to
    True.

- fullscreen (boolean; optional):
    Boolean that determines if the loading spinner will be displayed
    full-screen or not.

- fullscreenClassName (string; optional):
    **DEPRECATED** - use `fullscreen_class_name` instead.  Often used
    with CSS to style elements with common properties.

- fullscreen_class_name (string; optional):
    Often used with CSS to style elements with common properties.

- fullscreen_style (dict; optional):
    Defines CSS styles for the container when fullscreen=True.

- show_initially (boolean; default True):
    Whether the Spinner should show on app start-up before the loading
    state has been determined. Default True.

- size (string; optional):
    The spinner size. Options are 'sm', and 'md'.

- spinnerClassName (string; optional):
    **DEPRECATED** - use `spinner_class_name` instead.  CSS class
    names to apply to the spinner.

- spinner_class_name (string; optional):
    CSS class names to apply to the spinner.

- spinner_style (dict; optional):
    Inline CSS styles to apply to the spinner.

- type (string; default 'border'):
    The type of spinner. Options 'border' and 'grow'. Default
    'border'."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Spinner'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, fullscreen_style=Component.UNDEFINED, spinner_style=Component.UNDEFINED, fullscreen_class_name=Component.UNDEFINED, fullscreenClassName=Component.UNDEFINED, spinner_class_name=Component.UNDEFINED, spinnerClassName=Component.UNDEFINED, color=Component.UNDEFINED, type=Component.UNDEFINED, size=Component.UNDEFINED, fullscreen=Component.UNDEFINED, delay_hide=Component.UNDEFINED, delay_show=Component.UNDEFINED, show_initially=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'color', 'delay_hide', 'delay_show', 'fullscreen', 'fullscreenClassName', 'fullscreen_class_name', 'fullscreen_style', 'show_initially', 'size', 'spinnerClassName', 'spinner_class_name', 'spinner_style', 'type']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'color', 'delay_hide', 'delay_show', 'fullscreen', 'fullscreenClassName', 'fullscreen_class_name', 'fullscreen_style', 'show_initially', 'size', 'spinnerClassName', 'spinner_class_name', 'spinner_style', 'type']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Spinner, self).__init__(children=children, **args)
