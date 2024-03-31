# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Button(Component):
    """A Button component.
A component for creating Bootstrap buttons with different style options. The
Button component can act as a HTML button, link (<a>) or can be used like a
dash_core_components style `Link` for navigating between pages of a Dash app.

Use the `n_clicks` prop to trigger callbacks when the button has been
clicked.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- active (boolean; optional):
    Whether button is in active state. Default: False.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- color (string; optional):
    Button color, options: primary, secondary, success, info, warning,
    danger, link. Default: primary.

- disabled (boolean; optional):
    Disable button (make unclickable). Default: False.

- download (string; optional):
    Indicates that the hyperlink is to be used for downloading a
    resource.

- external_link (boolean; optional):
    If True, the browser will treat this as an external link, forcing
    a page refresh at the new location. If False, this just changes
    the location without triggering a page refresh. Use this if you
    are observing dcc.Location, for instance. Defaults to True for
    absolute URLs and False otherwise.

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
    Use of *_timestamp props has been deprecated in Dash in favour of
    dash.callback_context. See \"How do I determine which Input has
    changed?\" in the Dash FAQs https://dash.plot.ly/faqs.  An integer
    that represents the time (in ms since 1970) at which n_clicks
    changed. This can be used to tell which button was changed most
    recently.

- name (string; optional):
    The name of the button, submitted as a pair with the button’s
    value as part of the form data.

- outline (boolean; optional):
    Set outline button style, which removes background images and
    colors for a lightweight style.

- rel (string; optional):
    Set the rel attribute when Button is being used as a Link.

- size (string; optional):
    Button size, options: 'lg', 'md', 'sm'.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- target (string; optional):
    Target attribute to pass on to link if using Button as an external
    link.

- title (string; optional):
    Sets the title attribute of the underlying HTML button.

- type (a value equal to: 'button', 'reset', 'submit'; optional):
    The default behavior of the button. Possible values are:
    \"button\", \"reset\", \"submit\". If left unspecified the default
    depends on usage: for buttons associated with a form (e.g. a
    dbc.Button inside a dbc.Form) the default is \"submit\". Otherwise
    the default is \"button\".

- value (string; optional):
    Defines the value associated with the button’s name when it’s
    submitted with the form data. This value is passed to the server
    in params when the form is submitted."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Button'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, style=Component.UNDEFINED, key=Component.UNDEFINED, href=Component.UNDEFINED, external_link=Component.UNDEFINED, n_clicks=Component.UNDEFINED, n_clicks_timestamp=Component.UNDEFINED, active=Component.UNDEFINED, color=Component.UNDEFINED, disabled=Component.UNDEFINED, size=Component.UNDEFINED, title=Component.UNDEFINED, outline=Component.UNDEFINED, loading_state=Component.UNDEFINED, target=Component.UNDEFINED, type=Component.UNDEFINED, download=Component.UNDEFINED, name=Component.UNDEFINED, value=Component.UNDEFINED, rel=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'active', 'className', 'class_name', 'color', 'disabled', 'download', 'external_link', 'href', 'key', 'loading_state', 'n_clicks', 'n_clicks_timestamp', 'name', 'outline', 'rel', 'size', 'style', 'target', 'title', 'type', 'value']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'active', 'className', 'class_name', 'color', 'disabled', 'download', 'external_link', 'href', 'key', 'loading_state', 'n_clicks', 'n_clicks_timestamp', 'name', 'outline', 'rel', 'size', 'style', 'target', 'title', 'type', 'value']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Button, self).__init__(children=children, **args)
