# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Offcanvas(Component):
    """An Offcanvas component.
Create a toggleable hidden sidebar using the Offcanvas component.
Toggle the visibility with the `is_open` prop.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- autoFocus (boolean; optional):
    **DEPRECATED** Use `autofocus` instead          Puts the focus on
    the modal when initialized.

- autofocus (boolean; optional):
    Puts the focus on the offcanvas when initialized.

- backdrop (boolean | a value equal to: 'static'; default True):
    Includes an offcanvas-backdrop element. Alternatively, specify
    'static' for a backdrop which doesn't close the modal on click.

- backdropClassName (string; optional):
    **DEPRECATED** - Use backdrop_class_name instead.  CSS class to
    apply to the backdrop.

- backdrop_class_name (string; optional):
    CSS class to apply to the backdrop.

- className (string; optional):
    **DEPRECATED** - Use class_name instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- close_button (boolean; default True):
    Specify whether the Component should contain a close button in the
    header.

- is_open (boolean; default False):
    Whether offcanvas is currently open.

- keyboard (boolean; optional):
    Close the offcanvas when escape key is pressed.

- labelledBy (string; optional):
    **DEPRECATED** Use `labelledby` instead  The ARIA labelledby
    attribute.

- labelledby (string; optional):
    The ARIA labelledby attribute.

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

- placement (a value equal to: 'start', 'end', 'top', 'bottom'; optional):
    Which side of the viewport the offcanvas will appear from.

- scrollable (boolean; optional):
    Allow body scrolling while offcanvas is open.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- title (a list of or a singular dash component, string or number; optional):
    The header title."""
    _children_props = ['title']
    _base_nodes = ['title', 'children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Offcanvas'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, labelledby=Component.UNDEFINED, labelledBy=Component.UNDEFINED, backdrop=Component.UNDEFINED, backdrop_class_name=Component.UNDEFINED, backdropClassName=Component.UNDEFINED, keyboard=Component.UNDEFINED, is_open=Component.UNDEFINED, placement=Component.UNDEFINED, scrollable=Component.UNDEFINED, autofocus=Component.UNDEFINED, autoFocus=Component.UNDEFINED, title=Component.UNDEFINED, close_button=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'autoFocus', 'autofocus', 'backdrop', 'backdropClassName', 'backdrop_class_name', 'className', 'class_name', 'close_button', 'is_open', 'keyboard', 'labelledBy', 'labelledby', 'loading_state', 'placement', 'scrollable', 'style', 'title']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'autoFocus', 'autofocus', 'backdrop', 'backdropClassName', 'backdrop_class_name', 'className', 'class_name', 'close_button', 'is_open', 'keyboard', 'labelledBy', 'labelledby', 'loading_state', 'placement', 'scrollable', 'style', 'title']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Offcanvas, self).__init__(children=children, **args)
