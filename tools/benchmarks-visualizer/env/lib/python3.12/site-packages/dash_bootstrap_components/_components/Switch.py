# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Switch(Component):
    """A Switch component.
Checklist is a component that encapsulates several checkboxes.
The values and labels of the checklist is specified in the `options`
property and the checked items are specified with the `value` property.
Each checkbox is rendered as an input / label pair. `Checklist` must be
given an `id` to work properly.

Keyword arguments:

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  The class of the
    container (div).

- class_name (string; optional):
    The class of the container (div).

- disabled (boolean; default False):
    Disable the Switch.

- inputClassName (string; default ''):
    **DEPRECATED** Use `input_class_name` instead.  The class of the
    <input> checkbox element.

- inputStyle (dict; optional):
    **DEPRECATED** Use `input_style` instead.  The style of the
    <input> checkbox element.

- input_class_name (string; default ''):
    The class of the <input> checkbox element.

- input_style (dict; optional):
    The style of the <input> checkbox element.

- label (a list of or a singular dash component, string or number; optional):
    The label of the <input> element.

- labelClassName (string; default ''):
    **DEPRECATED** Use `label_class_name` instead.  CSS classes to
    apply to the <label> element for each item.

- labelStyle (dict; optional):
    **DEPRECATED** Use `label_style` instead.  Inline style arguments
    to apply to the <label> element for each item.

- label_class_name (string; default ''):
    CSS classes to apply to the <label> element for each item.

- label_id (string; optional):
    The id of the label.

- label_style (dict; optional):
    Inline style arguments to apply to the <label> element for each
    item.

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

- name (string; optional):
    The name of the control, which is submitted with the form data.

- persisted_props (list of a value equal to: 'value's; default ['value']):
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
    The style of the container (div).

- value (boolean; default False):
    The value of the input."""
    _children_props = ['label']
    _base_nodes = ['label', 'children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Switch'
    @_explicitize_args
    def __init__(self, id=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, style=Component.UNDEFINED, input_style=Component.UNDEFINED, inputStyle=Component.UNDEFINED, input_class_name=Component.UNDEFINED, inputClassName=Component.UNDEFINED, label=Component.UNDEFINED, label_id=Component.UNDEFINED, label_style=Component.UNDEFINED, labelStyle=Component.UNDEFINED, label_class_name=Component.UNDEFINED, labelClassName=Component.UNDEFINED, name=Component.UNDEFINED, value=Component.UNDEFINED, disabled=Component.UNDEFINED, loading_state=Component.UNDEFINED, persistence=Component.UNDEFINED, persisted_props=Component.UNDEFINED, persistence_type=Component.UNDEFINED, **kwargs):
        self._prop_names = ['id', 'className', 'class_name', 'disabled', 'inputClassName', 'inputStyle', 'input_class_name', 'input_style', 'label', 'labelClassName', 'labelStyle', 'label_class_name', 'label_id', 'label_style', 'loading_state', 'name', 'persisted_props', 'persistence', 'persistence_type', 'style', 'value']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['id', 'className', 'class_name', 'disabled', 'inputClassName', 'inputStyle', 'input_class_name', 'input_style', 'label', 'labelClassName', 'labelStyle', 'label_class_name', 'label_id', 'label_style', 'loading_state', 'name', 'persisted_props', 'persistence', 'persistence_type', 'style', 'value']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args}

        super(Switch, self).__init__(**args)
