# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Accordion(Component):
    """An Accordion component.
A self contained Accordion component. Build up the children using the
AccordionItem component.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- active_item (string | list of strings; optional):
    The item_id of the currently active item. If item_id has not been
    specified for the active item, this will default to item-i, where
    i is the index (starting from 0) of the item.  If
    `always_open=True`, this needs to be a list of string IDs.

- always_open (boolean; default False):
    You can make accordion items stay open when another item is opened
    by using the always_open prop.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- flush (boolean; optional):
    Renders accordion edge-to-edge with its parent container.

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

- persisted_props (list of a value equal to: 'active_item's; default ['active_item']):
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

- start_collapsed (boolean; default False):
    Set to True for all items to be collapsed initially.

- style (dict; optional):
    Defines CSS styles which will override styles previously set."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Accordion'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, flush=Component.UNDEFINED, active_item=Component.UNDEFINED, always_open=Component.UNDEFINED, start_collapsed=Component.UNDEFINED, loading_state=Component.UNDEFINED, persistence=Component.UNDEFINED, persisted_props=Component.UNDEFINED, persistence_type=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'active_item', 'always_open', 'className', 'class_name', 'flush', 'key', 'loading_state', 'persisted_props', 'persistence', 'persistence_type', 'start_collapsed', 'style']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'active_item', 'always_open', 'className', 'class_name', 'flush', 'key', 'loading_state', 'persisted_props', 'persistence', 'persistence_type', 'start_collapsed', 'style']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Accordion, self).__init__(children=children, **args)
