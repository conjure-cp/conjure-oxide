# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Select(Component):
    """A Select component.
Create a HTML select element with Bootstrap styles. Specify options as a
list of dictionaries with keys label, value and disabled.

Keyword arguments:

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- disabled (boolean; optional):
    Set to True to disable the Select.

- html_size (string; optional):
    This represents the number of rows in the select that should be
    visible at one time. It will result in the Select being rendered
    as a scrolling list box rather than a dropdown.

- invalid (boolean; optional):
    Apply invalid style to the Input for feedback purposes. This will
    cause any FormFeedback in the enclosing div with valid=False to
    display.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- name (string; optional):
    The name of the control, which is submitted with the form data.

- options (list of dicts; optional):
    The options to display as items in the component. This can be an
    array or a dictionary as follows:  \n1. Array of options where the
    label and the value are the same thing - [string|number]  \n2. An
    array of options ``` {   \"label\": [string|number],   \"value\":
    [string|number],   \"disabled\": [bool] (Optional),   \"title\":
    [string] (Optional) } ```  \n3. Simpler `options` representation
    in dictionary format. The order is not guaranteed. All values and
    labels will be treated as strings. ``` {\"value1\": \"label1\",
    \"value2\": \"label2\", ... } ``` which is equal to ``` [
    {\"label\": \"label1\", \"value\": \"value1\"},   {\"label\":
    \"label2\", \"value\": \"value2\"}, ... ] ```.

    `options` is a list of string | numbers | dict | list of dicts
    with keys:

    - disabled (boolean; optional):
        If True, this checkbox is disabled and can't be clicked on.

    - label (string | number; required):
        The options's label.

    - title (string; optional):
        The HTML 'title' attribute for the option. Allows for
        information on hover. For more information on this attribute,
        see
        https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/title.

    - value (string; required):
        The value of the option. This value corresponds to the items
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

- placeholder (string; default ''):
    Placeholder text to display before a selection is made.

- required (a value equal to: 'required', 'REQUIRED' | boolean; optional):
    This attribute specifies that the user must fill in a value before
    submitting a form. It cannot be used when the type attribute is
    hidden, image, or a button type (submit, reset, or button). The
    :optional and :required CSS pseudo-classes will be applied to the
    field as appropriate. required is an HTML boolean attribute - it
    is enabled by a boolean or 'required'. Alternative capitalizations
    `REQUIRED` are also acccepted.

- size (string; optional):
    Set the size of the Input. Options: 'sm' (small), 'md' (medium) or
    'lg' (large). Default is 'md'.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- valid (boolean; optional):
    Apply valid style to the Input for feedback purposes. This will
    cause any FormFeedback in the enclosing div with valid=True to
    display.

- value (string | number; default ''):
    The value of the currently selected option."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Select'
    @_explicitize_args
    def __init__(self, options=Component.UNDEFINED, value=Component.UNDEFINED, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, placeholder=Component.UNDEFINED, disabled=Component.UNDEFINED, required=Component.UNDEFINED, valid=Component.UNDEFINED, invalid=Component.UNDEFINED, size=Component.UNDEFINED, html_size=Component.UNDEFINED, persistence=Component.UNDEFINED, persisted_props=Component.UNDEFINED, persistence_type=Component.UNDEFINED, name=Component.UNDEFINED, **kwargs):
        self._prop_names = ['id', 'className', 'class_name', 'disabled', 'html_size', 'invalid', 'key', 'name', 'options', 'persisted_props', 'persistence', 'persistence_type', 'placeholder', 'required', 'size', 'style', 'valid', 'value']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['id', 'className', 'class_name', 'disabled', 'html_size', 'invalid', 'key', 'name', 'options', 'persisted_props', 'persistence', 'persistence_type', 'placeholder', 'required', 'size', 'style', 'valid', 'value']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args}

        super(Select, self).__init__(**args)
