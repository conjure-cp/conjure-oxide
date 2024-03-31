# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class DropdownMenuItem(Component):
    """A DropdownMenuItem component.
Use DropdownMenuItem to build up the content of a DropdownMenu.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- active (boolean; optional):
    Style this item as 'active'.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- disabled (boolean; optional):
    Style this item as 'disabled'.

- divider (boolean; optional):
    Set to True if this entry is a divider. Typically, it will have no
    children.

- external_link (boolean; optional):
    If True, the browser will treat this as an external link, forcing
    a page refresh at the new location. If False, this just changes
    the location without triggering a page refresh. Use this if you
    are observing dcc.Location, for instance. Defaults to True for
    absolute URLs and False otherwise.

- header (boolean; optional):
    Set to True if this is a header, rather than a conventional menu
    item.

- href (string; optional):
    Pass a URL (relative or absolute) to make the menu entry a link.

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

- n_clicks (number; default 0):
    An integer that represents the number of times that this element
    has been clicked on.

- n_clicks_timestamp (number; default -1):
    An integer that represents the time (in ms since 1970) at which
    n_clicks changed. This can be used to tell which button was
    changed most recently.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- target (string; optional):
    Target attribute to pass on to the link. Only applies to external
    links.

- toggle (boolean; default True):
    Whether to toggle the DropdownMenu on click. Default: True."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'DropdownMenuItem'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, active=Component.UNDEFINED, disabled=Component.UNDEFINED, divider=Component.UNDEFINED, header=Component.UNDEFINED, href=Component.UNDEFINED, toggle=Component.UNDEFINED, external_link=Component.UNDEFINED, n_clicks=Component.UNDEFINED, n_clicks_timestamp=Component.UNDEFINED, loading_state=Component.UNDEFINED, target=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'active', 'className', 'class_name', 'disabled', 'divider', 'external_link', 'header', 'href', 'key', 'loading_state', 'n_clicks', 'n_clicks_timestamp', 'style', 'target', 'toggle']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'active', 'className', 'class_name', 'disabled', 'divider', 'external_link', 'header', 'href', 'key', 'loading_state', 'n_clicks', 'n_clicks_timestamp', 'style', 'target', 'toggle']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(DropdownMenuItem, self).__init__(children=children, **args)
