# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Tab(Component):
    """A Tab component.
Create a single tab. Should be used as a component of Tabs.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- activeLabelClassName (string; optional):
    **DEPRECATED** Use `active_label_class_name` instead  Often used
    with CSS to style elements with common properties. The classes
    specified with this prop will be applied to the NavLink in the tab
    when it is active.

- activeTabClassName (string; optional):
    **DEPRECATED** Use `active_tab_class_name` instead  Often used
    with CSS to style elements with common properties. The classes
    specified with this prop will be applied to the NavItem in the tab
    when it is active.

- active_label_class_name (string; optional):
    Often used with CSS to style elements with common properties. The
    classes specified with this prop will be applied to the NavLink in
    the tab when it is active.

- active_label_style (dict; optional):
    Defines CSS styles which will override styles previously set. The
    styles set here apply to the NavLink in the tab when it is active.

- active_tab_class_name (string; optional):
    Often used with CSS to style elements with common properties. The
    classes specified with this prop will be applied to the NavItem in
    the tab when it is active.

- active_tab_style (dict; optional):
    Defines CSS styles which will override styles previously set. The
    styles set here apply to the NavItem in the tab when it is active.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- disabled (boolean; default False):
    Determines if tab is disabled or not - defaults to False.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- label (string; optional):
    The tab's label, displayed in the tab itself.

- labelClassName (string; optional):
    **DEPRECATED** Use `label_class_name` instead  Often used with CSS
    to style elements with common properties. The classes specified
    with this prop will be applied to the NavLink in the tab.

- label_class_name (string; optional):
    Often used with CSS to style elements with common properties. The
    classes specified with this prop will be applied to the NavLink in
    the tab.

- label_style (dict; optional):
    Defines CSS styles which will override styles previously set. The
    styles set here apply to the NavLink in the tab.

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

- style (dict; optional):
    Defines CSS styles which will override styles previously set. The
    styles set here apply to the content of the Tab.

- tabClassName (string; optional):
    **DEPRECATED** Use `tab_class_name` instead  Often used with CSS
    to style elements with common properties. The classes specified
    with this prop will be applied to the NavItem in the tab.

- tab_class_name (string; optional):
    Often used with CSS to style elements with common properties. The
    classes specified with this prop will be applied to the NavItem in
    the tab.

- tab_id (string; optional):
    Optional identifier for tab used for determining which tab is
    visible if not specified, and Tab is being used inside Tabs
    component, the tabId will be set to \"tab-i\" where i is (zero
    indexed) position of tab in list tabs pased to Tabs component.

- tab_style (dict; optional):
    Defines CSS styles which will override styles previously set. The
    styles set here apply to the NavItem in the tab."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Tab'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, tab_style=Component.UNDEFINED, active_tab_style=Component.UNDEFINED, label_style=Component.UNDEFINED, active_label_style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, tab_class_name=Component.UNDEFINED, tabClassName=Component.UNDEFINED, active_tab_class_name=Component.UNDEFINED, activeTabClassName=Component.UNDEFINED, label_class_name=Component.UNDEFINED, labelClassName=Component.UNDEFINED, active_label_class_name=Component.UNDEFINED, activeLabelClassName=Component.UNDEFINED, key=Component.UNDEFINED, label=Component.UNDEFINED, tab_id=Component.UNDEFINED, disabled=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'activeLabelClassName', 'activeTabClassName', 'active_label_class_name', 'active_label_style', 'active_tab_class_name', 'active_tab_style', 'className', 'class_name', 'disabled', 'key', 'label', 'labelClassName', 'label_class_name', 'label_style', 'loading_state', 'style', 'tabClassName', 'tab_class_name', 'tab_id', 'tab_style']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'activeLabelClassName', 'activeTabClassName', 'active_label_class_name', 'active_label_style', 'active_tab_class_name', 'active_tab_style', 'className', 'class_name', 'disabled', 'key', 'label', 'labelClassName', 'label_class_name', 'label_style', 'loading_state', 'style', 'tabClassName', 'tab_class_name', 'tab_id', 'tab_style']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Tab, self).__init__(children=children, **args)
