# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Toast(Component):
    """A Toast component.
Toasts can be used to push messages and notifactions to users. Control
visibility of the toast with the `is_open` prop, or use `duration` to set a
timer for auto-dismissal.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- bodyClassName (string; optional):
    **DEPRECATED** - use `body_class_name` instead.  Often used with
    CSS to style elements with common properties. The classes
    specified with this prop will be applied to the body of the toast.

- body_class_name (string; optional):
    Often used with CSS to style elements with common properties. The
    classes specified with this prop will be applied to the body of
    the toast.

- body_style (dict; optional):
    Defines CSS styles which will override styles previously set. The
    styles set here apply to the body of the toast.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- color (string; optional):
    Toast color, options: primary, secondary, success, info, warning,
    danger, light, dark. Default: secondary.

- dismissable (boolean; default False):
    Set to True to add a dismiss button to the header which will close
    the toast on click.

- duration (number; optional):
    Duration in milliseconds after which the Alert dismisses itself.

- header (a list of or a singular dash component, string or number; optional):
    Text to populate the header with.

- headerClassName (string; optional):
    **DEPRECATED** - use `header_class_name` instead  Often used with
    CSS to style elements with common properties. The classes
    specified with this prop will be applied to the header of the
    toast.

- header_class_name (string; optional):
    Often used with CSS to style elements with common properties. The
    classes specified with this prop will be applied to the header of
    the toast.

- header_style (dict; optional):
    Defines CSS styles which will override styles previously set. The
    styles set here apply to the header of the toast.

- icon (string; optional):
    Add a contextually coloured icon to the header of the toast.
    Options are: \"primary\", \"secondary\", \"success\", \"warning\",
    \"danger\", \"info\", \"light\" or \"dark\".

- is_open (boolean; default True):
    Whether Toast is currently open.

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

- n_dismiss (number; default 0):
    An integer that represents the number of times that the dismiss
    button has been clicked on.

- n_dismiss_timestamp (number; default -1):
    Use of *_timestamp props has been deprecated in Dash in favour of
    dash.callback_context. See \"How do I determine which Input has
    changed?\" in the Dash FAQs https://dash.plot.ly/faqs.  An integer
    that represents the time (in ms since 1970) at which n_dismiss
    changed. This can be used to tell which button was changed most
    recently.

- persisted_props (list of a value equal to: 'is_open's; default ['is_open']):
    Properties whose user interactions will persist after refreshing
    the component or the page. Since only `value` is allowed this prop
    can normally be ignored.

- persistence (boolean | string | number; optional):
    Used to allow user interactions in this component to be persisted
    when the component - or the page - is refreshed. If `persisted` is
    truthy and hasn't changed from its previous value, a `value` that
    the user has changed while using the app will keep that change, as
    long as the new `value` also matches what was given originally.
    Used in conjunction with `persistence_type`.

- persistence_type (a value equal to: 'local', 'session', 'memory'; default 'local'):
    Where persisted user changes will be stored: memory: only kept in
    memory, reset on page refresh. local: window.localStorage, data is
    kept after the browser quit. session: window.sessionStorage, data
    is cleared once the browser quit.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- tag (string; optional):
    HTML tag to use for the Toast, default: div."""
    _children_props = ['header']
    _base_nodes = ['header', 'children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Toast'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, header_style=Component.UNDEFINED, header_class_name=Component.UNDEFINED, headerClassName=Component.UNDEFINED, body_style=Component.UNDEFINED, body_class_name=Component.UNDEFINED, bodyClassName=Component.UNDEFINED, tag=Component.UNDEFINED, is_open=Component.UNDEFINED, key=Component.UNDEFINED, header=Component.UNDEFINED, dismissable=Component.UNDEFINED, duration=Component.UNDEFINED, n_dismiss=Component.UNDEFINED, n_dismiss_timestamp=Component.UNDEFINED, icon=Component.UNDEFINED, color=Component.UNDEFINED, loading_state=Component.UNDEFINED, persistence=Component.UNDEFINED, persisted_props=Component.UNDEFINED, persistence_type=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'bodyClassName', 'body_class_name', 'body_style', 'className', 'class_name', 'color', 'dismissable', 'duration', 'header', 'headerClassName', 'header_class_name', 'header_style', 'icon', 'is_open', 'key', 'loading_state', 'n_dismiss', 'n_dismiss_timestamp', 'persisted_props', 'persistence', 'persistence_type', 'style', 'tag']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'bodyClassName', 'body_class_name', 'body_style', 'className', 'class_name', 'color', 'dismissable', 'duration', 'header', 'headerClassName', 'header_class_name', 'header_style', 'icon', 'is_open', 'key', 'loading_state', 'n_dismiss', 'n_dismiss_timestamp', 'persisted_props', 'persistence', 'persistence_type', 'style', 'tag']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Toast, self).__init__(children=children, **args)
