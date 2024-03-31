# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Popover(Component):
    """A Popover component.
Popover creates a toggleable overlay that can be used to provide additional
information or content to users without having to load a new page or open a
new window.

Use the `PopoverHeader` and `PopoverBody` components to control the layout
of the children.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- autohide (boolean; default False):
    Optionally hide popover when hovering over content - default
    False.

- body (boolean; optional):
    When body is `True`, the Popover will render all children in a
    `PopoverBody` automatically.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- delay (dict; default {show: 0, hide: 50}):
    Optionally override show/hide delays.

    `delay` is a dict with keys:

    - hide (number; optional)

    - show (number; optional) | number

- flip (boolean; default True):
    Whether to flip the direction of the popover if too close to the
    container edge, default True.

- hide_arrow (boolean; optional):
    Hide popover arrow.

- innerClassName (string; optional):
    **DEPRECATED** Use `inner_class_name` instead.  CSS class to apply
    to the popover.

- inner_class_name (string; optional):
    CSS class to apply to the popover.

- is_open (boolean; optional):
    Whether the Popover is open or not.

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

- offset (string | number; optional):
    Offset of the popover relative to its target. The offset can be
    passed as a comma separated pair of values e.g. \"0,8\", where the
    first number, skidding, displaces the popover along the reference
    element. The second number, distance, displaces the popover away
    from, or toward, the reference element in the direction of its
    placement. A positive number displaces it further away, while a
    negative number lets it overlap the reference. See
    https://popper.js.org/docs/v2/modifiers/offset/ for more info.
    Alternatively, you can provide just a single 'distance' number
    e.g. 8 to displace it horizontally.

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

- placement (a value equal to: 'auto', 'auto-start', 'auto-end', 'top', 'top-start', 'top-end', 'right', 'right-start', 'right-end', 'bottom', 'bottom-start', 'bottom-end', 'left', 'left-start', 'left-end'; default 'right'):
    Specify popover placement.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- target (string | dict; optional):
    ID of the component to attach the popover to.

- trigger (string; optional):
    Space separated list of triggers (e.g. \"click hover focus
    legacy\"). These specify ways in which the target component can
    toggle the popover. If not specified you must toggle the popover
    yourself using callbacks. Options are: - \"click\": toggles the
    popover when the target is clicked. - \"hover\": toggles the
    popover when the target is hovered over with the cursor. -
    \"focus\": toggles the popover when the target receives focus -
    \"legacy\": toggles the popover when the target is clicked, but
    will also dismiss the popover when the user clicks outside of the
    popover."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Popover'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, placement=Component.UNDEFINED, target=Component.UNDEFINED, trigger=Component.UNDEFINED, is_open=Component.UNDEFINED, hide_arrow=Component.UNDEFINED, inner_class_name=Component.UNDEFINED, innerClassName=Component.UNDEFINED, delay=Component.UNDEFINED, offset=Component.UNDEFINED, flip=Component.UNDEFINED, body=Component.UNDEFINED, autohide=Component.UNDEFINED, loading_state=Component.UNDEFINED, persistence=Component.UNDEFINED, persisted_props=Component.UNDEFINED, persistence_type=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'autohide', 'body', 'className', 'class_name', 'delay', 'flip', 'hide_arrow', 'innerClassName', 'inner_class_name', 'is_open', 'key', 'loading_state', 'offset', 'persisted_props', 'persistence', 'persistence_type', 'placement', 'style', 'target', 'trigger']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'autohide', 'body', 'className', 'class_name', 'delay', 'flip', 'hide_arrow', 'innerClassName', 'inner_class_name', 'is_open', 'key', 'loading_state', 'offset', 'persisted_props', 'persistence', 'persistence_type', 'placement', 'style', 'target', 'trigger']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Popover, self).__init__(children=children, **args)
