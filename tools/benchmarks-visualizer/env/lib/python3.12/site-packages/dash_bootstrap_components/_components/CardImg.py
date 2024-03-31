# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class CardImg(Component):
    """A CardImg component.
Use CardImg to add images to your cards.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- alt (string; optional):
    Alternative text in case an image can't be displayed.

- bottom (boolean; optional):
    Set to True if image is at bottom of card. This will apply the
    card-img-bottom class which rounds the bottom corners to match the
    corners of the card.

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

- src (string; optional):
    The URI of the embeddable content.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- tag (string; optional):
    HTML tag to use for the card body, default: div.

- title (string; optional):
    Text to be displayed as a tooltip when hovering.

- top (boolean; optional):
    Set to True if image is at top of card. This will apply the
    card-img-top class which rounds the top corners to match the
    corners of the card."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'CardImg'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, tag=Component.UNDEFINED, top=Component.UNDEFINED, bottom=Component.UNDEFINED, src=Component.UNDEFINED, alt=Component.UNDEFINED, title=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'alt', 'bottom', 'className', 'class_name', 'key', 'loading_state', 'src', 'style', 'tag', 'title', 'top']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'alt', 'bottom', 'className', 'class_name', 'key', 'loading_state', 'src', 'style', 'tag', 'title', 'top']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(CardImg, self).__init__(children=children, **args)
