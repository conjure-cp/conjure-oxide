# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Navbar(Component):
    """A Navbar component.
The Navbar component can be used to make fully customisable navbars.

Keyword arguments:

- children (a list of or a singular dash component, string or number; optional):
    The children of this component.

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- color (string; default 'light'):
    Sets the color of the Navbar. Main options are primary, light and
    dark, default light.  You can also choose one of the other
    contextual classes provided by Bootstrap (secondary, success,
    warning, danger, info, white) or any valid CSS color of your
    choice (e.g. a hex code, a decimal code or a CSS color name).

- dark (boolean; optional):
    Applies the `navbar-dark` class to the Navbar, causing text in the
    children of the Navbar to use light colors for contrast /
    visibility.

- expand (boolean | string; default 'md'):
    Specify screen size at which to expand the menu bar, e.g. sm, md,
    lg etc.

- fixed (string; optional):
    Fix the navbar's position at the top or bottom of the page,
    options: top, bottom.

- key (string; optional):
    A unique identifier for the component, used to improve performance
    by React.js while rendering components See
    https://reactjs.org/docs/lists-and-keys.html for more info.

- light (boolean; default True):
    Applies the `navbar-light` class to the Navbar, causing text in
    the children of the Navbar to use dark colors for contrast /
    visibility.

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

- role (string; optional):
    The ARIA role attribute.

- sticky (a value equal to: 'top'; optional):
    Position the navbar at the top of the viewport, but only after
    scrolling past it. A convenience prop for the sticky-top
    positioning class. Not supported in <= IE11 and other older
    browsers  With `sticky`, the navbar remains in the viewport when
    you scroll. By contrast, with `fixed`, the navbar will remain at
    the top or bottom of the page.  sticky='top'.

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- tag (string; optional):
    HTML tag to use for the Navbar, default 'nav'."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Navbar'
    @_explicitize_args
    def __init__(self, children=None, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, key=Component.UNDEFINED, light=Component.UNDEFINED, dark=Component.UNDEFINED, fixed=Component.UNDEFINED, sticky=Component.UNDEFINED, color=Component.UNDEFINED, role=Component.UNDEFINED, tag=Component.UNDEFINED, expand=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['children', 'id', 'className', 'class_name', 'color', 'dark', 'expand', 'fixed', 'key', 'light', 'loading_state', 'role', 'sticky', 'style', 'tag']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['children', 'id', 'className', 'class_name', 'color', 'dark', 'expand', 'fixed', 'key', 'light', 'loading_state', 'role', 'sticky', 'style', 'tag']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args if k != 'children'}

        super(Navbar, self).__init__(children=children, **args)
