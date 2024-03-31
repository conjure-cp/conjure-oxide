# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Pagination(Component):
    """A Pagination component.
The container for presentational components for building a pagination UI.
Individual pages should be added as children using the `PaginationItem`
component.

Keyword arguments:

- id (string; optional):
    The ID of this component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- active_page (number; default 1):
    The currently active page.

- className (string; optional):
    **DEPRECATED** - Use class_name instead.  Often used with CSS to
    style elements with common properties.

- class_name (string; optional):
    Often used with CSS to style elements with common properties.

- first_last (boolean; default False):
    When True, this will display a first and last icon at the
    beginning and end of the component.

- fully_expanded (boolean; default True):
    When True, this will display all numbers between `min_value` and
    `max_value`.

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

- max_value (number; required):
    Maximum (rightmost) value to appear in the pagination. Must be
    defined. If the `min_value` and `step` together cannot reach this
    value, then the next stepped value is used as the maximum.

- min_value (number; default 1):
    Minimum (leftmost) value to appear in the pagination.

- previous_next (boolean; default False):
    When True, this will display a previous and next icon before and
    after the individual page numbers.

- size (a value equal to: 'sm', 'lg'; optional):
    Set the size of all page items in the pagination.

- step (number; default 1):
    Page increment step.

- style (dict; optional):
    Defines CSS styles which will override styles previously set."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Pagination'
    @_explicitize_args
    def __init__(self, id=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, style=Component.UNDEFINED, size=Component.UNDEFINED, min_value=Component.UNDEFINED, max_value=Component.REQUIRED, step=Component.UNDEFINED, active_page=Component.UNDEFINED, fully_expanded=Component.UNDEFINED, previous_next=Component.UNDEFINED, first_last=Component.UNDEFINED, loading_state=Component.UNDEFINED, **kwargs):
        self._prop_names = ['id', 'active_page', 'className', 'class_name', 'first_last', 'fully_expanded', 'loading_state', 'max_value', 'min_value', 'previous_next', 'size', 'step', 'style']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['id', 'active_page', 'className', 'class_name', 'first_last', 'fully_expanded', 'loading_state', 'max_value', 'min_value', 'previous_next', 'size', 'step', 'style']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args}

        for k in ['max_value']:
            if k not in args:
                raise TypeError(
                    'Required argument `' + k + '` was not specified.')

        super(Pagination, self).__init__(**args)
