# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Card(Component):
    """A Card component.
Component for creating Bootstrap cards. Use in conjunction with CardBody,
CardImg, CardLink, CardHeader and CardFooter. Can also be used in
conjunction with CardColumns, CardDeck, CardGroup for different layout
options.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- body (boolean; optional):
    Apply the `card-body` class to the card, so that there is no need
    to also include a CardBody component in the children of this Card.
    Default: False.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- color (string; optional):
    Card color, options: primary, secondary, success, info, warning,
    danger, light, dark or any valid CSS color of your choice (e.g. a
    hex code, a decimal code or a CSS color name). Default is light.

- inverse (boolean; optional):
    Invert text colours for use with a darker background.

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

- outline (boolean; optional):
    Apply color styling to just the border of the card.

- style (dict; optional):
    Defines CSS styles which will override styles previously set."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Card'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, color=Component.UNDEFINED, body=Component.UNDEFINED, outline=Component.UNDEFINED, inverse=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'body', 'className', 'class_name', 'color', 'inverse', 'key', 'loading_state', 'outline', 'style']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'body', 'className', 'class_name', 'color', 'inverse', 'key', 'loading_state', 'outline', 'style']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Card, self).__init__(children=children, **args)
