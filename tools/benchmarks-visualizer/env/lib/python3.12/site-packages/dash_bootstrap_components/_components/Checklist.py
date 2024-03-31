# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Checklist(Component):
    """A Checklist component.
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

- inline (boolean; optional):
    Arrange Checklist inline.

- inputCheckedClassName (string; optional):
    **DEPRECATED** Use `input_checked_class_name` instead.  Additional
    CSS classes to apply to the <input> element when the corresponding
    checkbox is checked.

- inputCheckedStyle (dict; optional):
    **DEPRECATED** Use `input_checked_style` instead.  Additional
    inline style arguments to apply to <input> elements on checked
    items.

- inputClassName (string; default ''):
    **DEPRECATED** Use `input_class_name` instead.  The class of the
    <input> checkbox element.

- inputStyle (dict; optional):
    **DEPRECATED** Use `input_style` instead.  The style of the
    <input> checkbox element.

- input_checked_class_name (string; optional):
    Additional CSS classes to apply to the <input> element when the
    corresponding checkbox is checked.

- input_checked_style (dict; optional):
    Additional inline style arguments to apply to <input> elements on
    checked items.

- input_class_name (string; default ''):
    The class of the <input> checkbox element.

- input_style (dict; optional):
    The style of the <input> checkbox element.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- labelCheckedClassName (string; optional):
    **DEPRECATED** Use `label_checked_class_name` instead.  Additional
    CSS classes to apply to the <label> element when the corresponding
    checkbox is checked.

- labelCheckedStyle (dict; optional):
    **DEPRECATED** Use `label_checked_style` instead.  Additional
    inline style arguments to apply to <label> elements on checked
    items.

- labelClassName (string; default ''):
    **DEPRECATED** Use `label_class_name` instead.  CSS classes to
    apply to the <label> element for each item.

- labelStyle (dict; optional):
    **DEPRECATED** Use `label_style` instead.  Inline style arguments
    to apply to the <label> element for each item.

- label_checked_class_name (string; optional):
    Additional CSS classes to apply to the <label> element when the
    corresponding checkbox is checked.

- label_checked_style (dict; optional):
    Additional inline style arguments to apply to <label> elements on
    checked items.

- label_class_name (string; default ''):
    CSS classes to apply to the <label> element for each item.

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

- options (list of dicts; optional):
    The options to display as items in the component. This can be an
    array or a dictionary as follows:  \n1. Array of options where the
    label and the value are the same thing - [string|number]  \n2. An
    array of options ``` {   \"label\": [string|number],   \"value\":
    [string|number],   \"disabled\": [bool] (Optional),
    \"input_id\": [string] (Optional),   \"label_id\": [string]
    (Optional) } ```  \n3. Simpler `options` representation in
    dictionary format. The order is not guaranteed. All values and
    labels will be treated as strings. ``` {\"value1\": \"label1\",
    \"value2\": \"label2\", ... } ``` which is equal to ``` [
    {\"label\": \"label1\", \"value\": \"value1\"},   {\"label\":
    \"label2\", \"value\": \"value2\"}, ... ] ```.

    `options` is a list of string | numbers | dict | list of dicts
    with keys:

    - disabled (boolean; optional):
        If True, this checkbox is disabled and can't be clicked on.

    - input_id (string; optional):
        id for this option's input, can be used to attach tooltips or
        apply CSS styles.

    - label (a list of or a singular dash component, string or number; required):
        The checkbox's label.

    - label_id (string; optional):
        id for this option's label, can be used to attach tooltips or
        apply CSS styles.

    - value (string | number; required):
        The value of the checkbox. This value corresponds to the items
        specified in the `value` property.

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

- switch (boolean; optional):
    Set to True to render toggle-like switches instead of checkboxes.

- value (list of string | numbers; optional):
    The currently selected value."""
    _children_props = ['options[].label']
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Checklist'
    @_explicitize_args
    def __init__(self, options=Component.UNDEFINED, value=Component.UNDEFINED, id=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, style=Component.UNDEFINED, key=Component.UNDEFINED, input_style=Component.UNDEFINED, inputStyle=Component.UNDEFINED, input_checked_style=Component.UNDEFINED, inputCheckedStyle=Component.UNDEFINED, input_class_name=Component.UNDEFINED, inputClassName=Component.UNDEFINED, input_checked_class_name=Component.UNDEFINED, inputCheckedClassName=Component.UNDEFINED, label_style=Component.UNDEFINED, labelStyle=Component.UNDEFINED, label_checked_style=Component.UNDEFINED, labelCheckedStyle=Component.UNDEFINED, label_class_name=Component.UNDEFINED, labelClassName=Component.UNDEFINED, label_checked_class_name=Component.UNDEFINED, labelCheckedClassName=Component.UNDEFINED, inline=Component.UNDEFINED, switch=Component.UNDEFINED, loading_state=Component.UNDEFINED, persistence=Component.UNDEFINED, persisted_props=Component.UNDEFINED, persistence_type=Component.UNDEFINED, name=Component.UNDEFINED, **kwargs):
        self._prop_names = ['id', 'className', 'class_name', 'inline', 'inputCheckedClassName', 'inputCheckedStyle', 'inputClassName', 'inputStyle', 'input_checked_class_name', 'input_checked_style', 'input_class_name', 'input_style', 'key', 'labelCheckedClassName', 'labelCheckedStyle', 'labelClassName', 'labelStyle', 'label_checked_class_name', 'label_checked_style', 'label_class_name', 'label_style', 'loading_state', 'name', 'options', 'persisted_props', 'persistence', 'persistence_type', 'style', 'switch', 'value']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['id', 'className', 'class_name', 'inline', 'inputCheckedClassName', 'inputCheckedStyle', 'inputClassName', 'inputStyle', 'input_checked_class_name', 'input_checked_style', 'input_class_name', 'input_style', 'key', 'labelCheckedClassName', 'labelCheckedStyle', 'labelClassName', 'labelStyle', 'label_checked_class_name', 'label_checked_style', 'label_class_name', 'label_style', 'loading_state', 'name', 'options', 'persisted_props', 'persistence', 'persistence_type', 'style', 'switch', 'value']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args}

        super(Checklist, self).__init__(**args)
