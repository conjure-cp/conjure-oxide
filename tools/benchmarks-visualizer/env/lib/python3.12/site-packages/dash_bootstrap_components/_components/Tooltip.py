# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Tooltip(Component):
    """A Tooltip component.
A component for adding tooltips to any element, no callbacks required!

Simply add the Tooltip to you layout, and give it a target (id of a
component to which the tooltip should be attached)

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- autohide (boolean; default True):
    Optionally hide tooltip when hovering over tooltip content -
    default True.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- delay (dict; default {show: 0, hide: 50}):
    Control the delay of hide and show events.

    `delay` is a dict with keys:

    - hide (number; optional)

    - show (number; optional)

- fade (boolean; default True):
    If True, a fade animation will be applied when `is_open` is
    toggled. If False the Alert will simply appear and disappear.

- flip (boolean; default True):
    Whether to flip the direction of the popover if too close to the
    container edge, default True.

- is_open (boolean; optional):
    Whether the Tooltip is open or not.

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

- placement (a value equal to: 'auto', 'auto-start', 'auto-end', 'top', 'top-start', 'top-end', 'right', 'right-start', 'right-end', 'bottom', 'bottom-start', 'bottom-end', 'left', 'left-start', 'left-end'; default 'auto'):
    How to place the tooltip.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- target (string | dict; optional):
    The id of the element to attach the tooltip to.

- trigger (string; default 'hover focus'):
    Space separated list of triggers (e.g. \"click hover focus
    legacy\"). These specify ways in which the target component can
    toggle the tooltip. If omitted you must toggle the tooltip
    yourself using callbacks. Options are: - \"click\": toggles the
    popover when the target is clicked. - \"hover\": toggles the
    popover when the target is hovered over with the cursor. -
    \"focus\": toggles the popover when the target receives focus -
    \"legacy\": toggles the popover when the target is clicked, but
    will also dismiss the popover when the user clicks outside of the
    popover.  Default is \"hover focus\"."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Tooltip'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, target=Component.UNDEFINED, placement=Component.UNDEFINED, flip=Component.UNDEFINED, delay=Component.UNDEFINED, autohide=Component.UNDEFINED, fade=Component.UNDEFINED, trigger=Component.UNDEFINED, is_open=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'autohide', 'className', 'class_name', 'delay', 'fade', 'flip', 'is_open', 'key', 'loading_state', 'placement', 'style', 'target', 'trigger']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'autohide', 'className', 'class_name', 'delay', 'fade', 'flip', 'is_open', 'key', 'loading_state', 'placement', 'style', 'target', 'trigger']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Tooltip, self).__init__(children=children, **args)
