# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Breadcrumb(Component):
    """A Breadcrumb component.
Use breadcrumbs to create a navigation breadcrumb in your app.

Keyword arguments:

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- className (string; optional):
    **DEPRECATED** - Use class_name instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- itemClassName (string; optional):
    **DEPRECATED** - use item_class_name instead.  Class name ot apply
    to each item.

- item_class_name (string; optional):
    Class name to apply to each item.

- item_style (dict; optional):
    Defines inline CSS styles that will be added to each item in the
    breadcrumbs.

- items (list of dicts; optional):
    The details of the items to render inside of this component.

    `items` is a list of dicts with keys:

    - active (boolean; optional):
        Apply 'active' style to this component.

    - external_link (boolean; optional):
        If True, the browser will treat this as an external link,
        forcing a page refresh at the new location. If False, this
        just changes the location without triggering a page refresh.
        Use this if you are observing dcc.Location, for instance.
        Defaults to True for absolute URLs and False otherwise.

    - href (string; optional):
        URL of the resource to link to.

    - label (string; optional):
        Label to display inside the breadcrumbs.

    - target (string; optional):
        Target attribute to pass on to the link. Only applies to
        external links.

    - title (string; optional):
        title attribute for the inner a element.

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

- style (dict; optional):
    Defines CSS styles which will override styles previously set.

- tag (dict; optional):
    HTML tag to use for the outer breadcrumb component. Default:
    \"nav\"."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Breadcrumb'
    @_explicitize_args
    def __init__(self, id=Component.UNDEFINED, items=Component.UNDEFINED, style=Component.UNDEFINED, item_style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, item_class_name=Component.UNDEFINED, itemClassName=Component.UNDEFINED, key=Component.UNDEFINED, tag=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['id', 'className', 'class_name', 'itemClassName', 'item_class_name', 'item_style', 'items', 'key', 'loading_state', 'style', 'tag']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['id', 'className', 'class_name', 'itemClassName', 'item_class_name', 'item_style', 'items', 'key', 'loading_state', 'style', 'tag']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args}

        super(Breadcrumb, self).__init__(**args)
