# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class NavLink(Component):
    """A NavLink component.
Add a link to a `Nav`. Can be used as a child of `NavItem` or of `Nav`
directly.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- active (boolean | a value equal to: 'partial', 'exact'; default False):
    Apply 'active' style to this component. Set to \"exact\" to
    automatically toggle active status when the current pathname
    matches href, or to \"partial\" to automatically toggle on a
    partial match. Assumes that href is a relative url such as /link
    rather than an absolute such as https://example.com/link  For
    example - dbc.NavLink(..., href=\"/my-page\", active=\"exact\")
    will be active on   \"/my-page\" but not \"/my-page/other\" or
    \"/random\" - dbc.NavLink(..., href=\"/my-page\",
    active=\"partial\") will be active on   \"/my-page\" and
    \"/my-page/other\" but not \"/random\".

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- disabled (boolean; default False):
    Disable the link.

- external_link (boolean; optional):
    If True, the browser will treat this as an external link, forcing
    a page refresh at the new location. If False, this just changes
    the location without triggering a page refresh. Use this if you
    are observing dcc.Location, for instance. Defaults to True for
    absolute URLs and False otherwise.

- href (string; optional):
    The URL of the linked resource.

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
    links."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'NavLink'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, href=Component.UNDEFINED, active=Component.UNDEFINED, disabled=Component.UNDEFINED, external_link=Component.UNDEFINED, n_clicks=Component.UNDEFINED, n_clicks_timestamp=Component.UNDEFINED, loading_state=Component.UNDEFINED, target=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'active', 'className', 'class_name', 'disabled', 'external_link', 'href', 'key', 'loading_state', 'n_clicks', 'n_clicks_timestamp', 'style', 'target']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'active', 'className', 'class_name', 'disabled', 'external_link', 'href', 'key', 'loading_state', 'n_clicks', 'n_clicks_timestamp', 'style', 'target']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(NavLink, self).__init__(children=children, **args)
