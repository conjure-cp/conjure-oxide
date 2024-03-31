# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class DropdownMenu(Component):
    """A DropdownMenu component.
DropdownMenu creates an overlay useful for grouping together links and other
content to organise navigation or other interactive elements.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- addon_type (boolean | a value equal to: 'prepend', 'append'; optional):
    Set this to 'prepend' or 'append' if the DropdownMenu is being
    used in an input group.

- align_end (boolean; optional):
    Align the DropdownMenu along the right side of its parent.
    Default: False.

- caret (boolean; default True):
    Add a caret to the DropdownMenu toggle. Default: True.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- color (string; optional):
    Set the color of the DropdownMenu toggle. Available options are:
    'primary', 'secondary', 'success', 'warning', 'danger', 'info',
    'link' or any valid CSS color of your choice (e.g. a hex code, a
    decimal code or a CSS color name) Default: 'primary'.

- direction (a value equal to: 'down', 'start', 'end', 'up', 'left', 'right', 'end'; optional):
    Direction in which to expand the DropdownMenu. Default: 'down'.
    `left` and `right` have been deprecated, and `start` and `end`
    should be used instead.

- disabled (boolean; default False):
    Disable the dropdown.

- group (boolean; optional):
    Set group to True if the DropdownMenu is inside a ButtonGroup.

- in_navbar (boolean; optional):
    Set this to True if the DropdownMenu is inside a navbar. Default:
    False.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- label (a list of or a singular dash component, string or number; optional):
    Label for the DropdownMenu toggle.

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

- menu_variant (a value equal to: 'light', 'dark'; default 'light'):
    Set `menu_variant=\"dark\"` to create a dark-mode drop down
    instead.

- nav (boolean; optional):
    Set this to True if the DropdownMenu is inside a nav for styling
    consistent with other nav items. Default: False.

- right (boolean; optional):
    **DEPRECATED** Use `align_end` instead.  Align the DropdownMenu
    along the right side of its parent. Default: False.

- size (a value equal to: 'sm', 'md', 'lg'; optional):
    Size of the DropdownMenu. 'sm' corresponds to small, 'md' to
    medium and 'lg' to large.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- toggleClassName (string; optional):
    **DEPRECATED** Use `toggle_class_name` instead.  Often used with
    CSS to style elements with common properties. The classes
    specified with this prop will be applied to the DropdownMenu
    toggle.

- toggle_class_name (string; optional):
    Often used with CSS to style elements with common properties. The
    classes specified with this prop will be applied to the
    DropdownMenu toggle.

- toggle_style (dict; optional):
    Defines CSS styles which will override styles previously set. The
    styles set here apply to the DropdownMenu toggle."""
    _children_props = ['label']
    _base_nodes = ['label', 'children']
    _namespace = 'dash_bootstrap_components'
    _type = 'DropdownMenu'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, label=Component.UNDEFINED, direction=Component.UNDEFINED, align_end=Component.UNDEFINED, right=Component.UNDEFINED, in_navbar=Component.UNDEFINED, addon_type=Component.UNDEFINED, disabled=Component.UNDEFINED, nav=Component.UNDEFINED, caret=Component.UNDEFINED, color=Component.UNDEFINED, menu_variant=Component.UNDEFINED, toggle_style=Component.UNDEFINED, toggle_class_name=Component.UNDEFINED, toggleClassName=Component.UNDEFINED, size=Component.UNDEFINED, loading_state=Component.UNDEFINED, group=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'addon_type', 'align_end', 'caret', 'className', 'class_name', 'color', 'direction', 'disabled', 'group', 'in_navbar', 'key', 'label', 'loading_state', 'menu_variant', 'nav', 'right', 'size', 'style', 'toggleClassName', 'toggle_class_name', 'toggle_style']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'addon_type', 'align_end', 'caret', 'className', 'class_name', 'color', 'direction', 'disabled', 'group', 'in_navbar', 'key', 'label', 'loading_state', 'menu_variant', 'nav', 'right', 'size', 'style', 'toggleClassName', 'toggle_class_name', 'toggle_style']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(DropdownMenu, self).__init__(children=children, **args)
