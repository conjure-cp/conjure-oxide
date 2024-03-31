# AUTO GENERATED FILE - DO NOT EDIT

from dash.development.base_component import Component, _explicitize_args


class Carousel(Component):
    """A Carousel component.
Component for creating Bootstrap carousel.  This component is a slideshow
for cycling through a series of content.

Keyword arguments:

- id (string; optional):
    The ID of the component, used to identify dash components in
    callbacks. The ID needs to be unique across all of the components
    in an app.

- active_index (number; default 0):
    The current visible slide number.

- className (string; optional):
    **DEPRECATED** Use `class_name` instead.  efines the className of
    the carousel container. Often used with CSS to style elements with
    common properties.

- class_name (string; optional):
    Defines the className of the carousel container. Often used with
    CSS to style elements with common properties.

- controls (boolean; default True):
    Show the Carousel previous and next arrows for changing the
    current slide.

- indicators (boolean; default True):
    Show a set of slide position indicators.

- interval (number; optional):
    the interval at which the carousel automatically cycles (default:
    5000) If set to None, carousel will not Autoplay (i.e. will not
    automatically cycle).

- items (list of dicts; required):
    The items to display on the slides in the carousel.

    `items` is a list of dicts with keys:

    - alt (string; optional):
        The alternate text for an image, if the image cannot be
        displayed.

    - caption (string; optional):
        The caption of the item.  The text is displayed in a <p>
        element.

    - captionClassName (string; optional):
        **DEPRECATED** Use `caption_class_name` instead.  The class
        name for the header and caption container.

    - caption_class_name (string; optional):
        The class name for the header and caption container.

    - external_link (boolean; optional):
        If True, the browser will treat this as an external link,
        forcing a page refresh at the new location. If False, this
        just changes the location without triggering a page refresh.
        Use this if you are observing dcc.Location, for instance.
        Defaults to True for absolute URLs and False otherwise.

    - header (string; optional):
        The header of the text on the slide. It is displayed in a <h5>
        element.

    - href (string; optional):
        Optional hyperlink to add to the item. Item will be rendered
        as a HTML <a> or as a Dash-style link depending on whether the
        link is deemed to be internal or external. Override this
        automatic detection with the external_link argument.

    - imgClassName (string; optional):
        **DEPRECATED** Use `img_class_name` instead.  The className
        for the image.  The default is 'd-block w-100'.

    - img_class_name (string; optional):
        The className for the image.  The default is 'd-block w-100'.

    - img_style (dict; optional):
        The style for the image.

    - key (string; optional):
        A unique identifier for the slide, used to improve performance
        by React.js while rendering components See
        https://reactjs.org/docs/lists-and-keys.html for more info.

    - src (string; optional):
        The URL of the image.

    - target (string; optional):
        Optional target attribute for the link. Only applies if `href`
        is set, default `_self`.

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

- persisted_props (list of a value equal to: 'active_index's; default ['active_index']):
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

- ride (a value equal to: 'carousel'; optional):
    Autoplays the carousel after the user manually cycles the first
    item. If \"carousel\", autoplays the carousel on load.

- slide (boolean; optional):
    controls whether the slide animation on the Carousel works or not.

- style (dict; optional):
    Defines CSS styles of the carousel container. Will override styles
    previously set.

- variant (a value equal to: 'dark'; optional):
    Add `variant=\"dark\"` to the Carousel for darker controls,
    indicators, and captions."""
    _children_props = []
    _base_nodes = ['children']
    _namespace = 'dash_bootstrap_components'
    _type = 'Carousel'
    @_explicitize_args
    def __init__(self, id=Component.UNDEFINED, style=Component.UNDEFINED, class_name=Component.UNDEFINED, className=Component.UNDEFINED, items=Component.REQUIRED, active_index=Component.UNDEFINED, controls=Component.UNDEFINED, indicators=Component.UNDEFINED, ride=Component.UNDEFINED, slide=Component.UNDEFINED, variant=Component.UNDEFINED, interval=Component.UNDEFINED, loading_state=Component.UNDEFINED, persistence=Component.UNDEFINED, persisted_props=Component.UNDEFINED, persistence_type=Component.UNDEFINED, **kwargs):
        self._prop_names = ['id', 'active_index', 'className', 'class_name', 'controls', 'indicators', 'interval', 'items', 'loading_state', 'persisted_props', 'persistence', 'persistence_type', 'ride', 'slide', 'style', 'variant']
        self._valid_wildcard_attributes =            []
        self.available_properties = ['id', 'active_index', 'className', 'class_name', 'controls', 'indicators', 'interval', 'items', 'loading_state', 'persisted_props', 'persistence', 'persistence_type', 'ride', 'slide', 'style', 'variant']
        self.available_wildcard_properties =            []
        _explicit_args = kwargs.pop('_explicit_args')
        _locals = locals()
        _locals.update(kwargs)  # For wildcard attrs and excess named props
        args = {k: _locals[k] for k in _explicit_args}

        for k in ['items']:
            if k not in args:
                raise TypeError(
                    'Required argument `' + k + '` was not specified.')

        super(Carousel, self).__init__(**args)
