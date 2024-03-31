# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Textarea(Component):
    """A Textarea component.
A basic HTML textarea for entering multiline text based on the corresponding
component in dash-core-components

Keyword arguments:

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- accessKey (string; optional):
    **DEPRECATED** Use `accesskey` instead  Defines a keyboard
    shortcut to activate or add focus to the element.

- accesskey (string; optional):
    Defines a keyboard shortcut to activate or add focus to the
    element.

- autoFocus (string; optional):
    **DEPRECATED** Use `autofocus` instead  The element should be
    automatically focused after the page loaded.

- autofocus (string; optional):
    The element should be automatically focused after the page loaded.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- cols (string | number; optional):
    Defines the number of columns in a textarea.

- contentEditable (string | number; optional):
    **DEPRECATED** Use `contenteditable` instead  Indicates whether
    the element's content is editable.

- contenteditable (string | number; optional):
    Indicates whether the element's content is editable.

- contextMenu (string; optional):
    **DEPRECATED** Use `contextmenu` instead  Defines the ID of a
    <menu> element which will serve as the element's context menu.

- contextmenu (string; optional):
    Defines the ID of a <menu> element which will serve as the
    element's context menu.

- debounce (boolean; default False):
    If True, changes to input will be sent back to the Dash server
    only on enter or when losing focus. If it's False, it will sent
    the value back on every change.

- dir (string; optional):
    Defines the text direction. Allowed values are ltr (Left-To-Right)
    or rtl (Right-To-Left).

- disabled (string | boolean; optional):
    Indicates whether the user can interact with the element.

- draggable (a value equal to: 'true', 'false' | boolean; optional):
    Defines whether the element can be dragged.

- form (string; optional):
    Indicates the form that is the owner of the element.

- hidden (string; optional):
    Prevents rendering of given element, while keeping child elements,
    e.g. script elements, active.

- invalid (boolean; optional):
    Apply invalid style to the Textarea for feedback purposes. This
    will cause any FormFeedback in the enclosing div with valid=False
    to display.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- lang (string; optional):
    Defines the language used in the element.

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

- maxLength (string | number; optional):
    **DEPRECATED** Use `maxlength` instead  Defines the maximum number
    of characters allowed in the element.

- maxlength (string | number; optional):
    Defines the maximum number of characters allowed in the element.

- minLength (string | number; optional):
    **DEPRECATED** Use `minlength` instead  Defines the minimum number
    of characters allowed in the element.

- minlength (string | number; optional):
    Defines the minimum number of characters allowed in the element.

- n_blur (number; default 0):
    Number of times the input lost focus.

- n_blur_timestamp (number; default -1):
    Last time the input lost focus.

- n_clicks (number; default 0):
    An integer that represents the number of times that this element
    has been clicked on.

- n_clicks_timestamp (number; default -1):
    An integer that represents the time (in ms since 1970) at which
    n_clicks changed. This can be used to tell which button was
    changed most recently.

- n_submit (number; default 0):
    Number of times the `Enter` key was pressed while the textarea had
    focus.

- n_submit_timestamp (number; default -1):
    Last time that `Enter` was pressed.

- name (string; optional):
    Name of the element. For example used by the server to identify
    the fields in form submits.

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

- placeholder (string; optional):
    Provides a hint to the user of what can be entered in the field.

- readOnly (boolean | a value equal to: 'readOnly', 'readonly', 'READONLY'; optional):
    **DEPRECATED** Use `readonly` instead  Indicates whether the
    element can be edited.

- readonly (boolean | a value equal to: 'readOnly', 'readonly', 'READONLY'; optional):
    Indicates whether the element can be edited.

- required (a value equal to: 'required', 'REQUIRED' | boolean; optional):
    This attribute specifies that the user must fill in a value before
    submitting a form. It cannot be used when the type attribute is
    hidden, image, or a button type (submit, reset, or button). The
    :optional and :required CSS pseudo-classes will be applied to the
    field as appropriate. required is an HTML boolean attribute - it
    is enabled by a boolean or 'required'. Alternative capitalizations
    `REQUIRED` are also acccepted.

- rows (string | number; optional):
    Defines the number of rows in a text area.

- size (string; optional):
    Set the size of the Textarea, valid options are 'sm', 'md', or
    'lg'.

- spellCheck (a value equal to: 'true', 'false' | boolean; optional):
    **DEPRECATED** Use `spellcheck` instead  Indicates whether spell
    checking is allowed for the element.

- spellcheck (a value equal to: 'true', 'false' | boolean; optional):
    Indicates whether spell checking is allowed for the element.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- tabIndex (string | number; optional):
    **DEPRECATED** Use `tabindex` instead  Overrides the browser's
    default tab order and follows the one specified instead.

- tabindex (string | number; optional):
    Overrides the browser's default tab order and follows the one
    specified instead.

- title (string; optional):
    Text to be displayed in a tooltip when hovering over the element.

- valid (boolean; optional):
    Apply valid style to the Textarea for feedback purposes. This will
    cause any FormFeedback in the enclosing div with valid=True to
    display.

- value (string; default ''):
    The value of the textarea.

- wrap (string; optional):
    Indicates whether the text should be wrapped."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Textarea'
    @_explicitize_args
    def __init__(self, id=Component.UNDEFINED, key=Component.UNDEFINED, value=Component.UNDEFINED, autofocus=Component.UNDEFINED, autoFocus=Component.UNDEFINED, cols=Component.UNDEFINED, disabled=Component.UNDEFINED, form=Component.UNDEFINED, maxlength=Component.UNDEFINED, maxLength=Component.UNDEFINED, minlength=Component.UNDEFINED, minLength=Component.UNDEFINED, name=Component.UNDEFINED, placeholder=Component.UNDEFINED, readonly=Component.UNDEFINED, readOnly=Component.UNDEFINED, required=Component.UNDEFINED, rows=Component.UNDEFINED, wrap=Component.UNDEFINED, accesskey=Component.UNDEFINED, accessKey=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, contenteditable=Component.UNDEFINED, contentEditable=Component.UNDEFINED, contextmenu=Component.UNDEFINED, contextMenu=Component.UNDEFINED, dir=Component.UNDEFINED, draggable=Component.UNDEFINED, hidden=Component.UNDEFINED, lang=Component.UNDEFINED, spellcheck=Component.UNDEFINED, spellCheck=Component.UNDEFINED, style=Component.UNDEFINED, tabindex=Component.UNDEFINED, tabIndex=Component.UNDEFINED, title=Component.UNDEFINED, size=Component.UNDEFINED, valid=Component.UNDEFINED, invalid=Component.UNDEFINED, n_blur=Component.UNDEFINED, n_blur_timestamp=Component.UNDEFINED, n_submit=Component.UNDEFINED, n_submit_timestamp=Component.UNDEFINED, n_clicks=Component.UNDEFINED, n_clicks_timestamp=Component.UNDEFINED, debounce=Component.UNDEFINED, loading_state=Component.UNDEFINED, persistence=Component.UNDEFINED, persisted_props=Component.UNDEFINED, persistence_type=Component.UNDEFINED, **kwargs):
        self._prop_names = ['id', 'accessKey', 'accesskey', 'autoFocus', 'autofocus', 'className', 'class_name', 'cols', 'contentEditable', 'contenteditable', 'contextMenu', 'contextmenu', 'debounce', 'dir', 'disabled', 'draggable', 'form', 'hidden', 'invalid', 'key', 'lang', 'loading_state', 'maxLength', 'maxlength', 'minLength', 'minlength', 'n_blur', 'n_blur_timestamp', 'n_clicks', 'n_clicks_timestamp', 'n_submit', 'n_submit_timestamp', 'name', 'persisted_props', 'persistence', 'persistence_type', 'placeholder', 'readOnly', 'readonly', 'required', 'rows', 'size', 'spellCheck', 'spellcheck', 'style', 'tabIndex', 'tabindex', 'title', 'valid', 'value', 'wrap']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['id', 'accessKey', 'accesskey', 'autoFocus', 'autofocus', 'className', 'class_name', 'cols', 'contentEditable', 'contenteditable', 'contextMenu', 'contextmenu', 'debounce', 'dir', 'disabled', 'draggable', 'form', 'hidden', 'invalid', 'key', 'lang', 'loading_state', 'maxLength', 'maxlength', 'minLength', 'minlength', 'n_blur', 'n_blur_timestamp', 'n_clicks', 'n_clicks_timestamp', 'n_submit', 'n_submit_timestamp', 'name', 'persisted_props', 'persistence', 'persistence_type', 'placeholder', 'readOnly', 'readonly', 'required', 'rows', 'size', 'spellCheck', 'spellcheck', 'style', 'tabIndex', 'tabindex', 'title', 'valid', 'value', 'wrap']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args}

        super(Textarea, self).__init__(**args)
