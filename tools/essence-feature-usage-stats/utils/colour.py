from collections.abc import Iterable

from utils.maths import clamp, map_range


class ColourConstructorError(ValueError):
    default_message = """
                     Supported inputs:
                     - Colour(r: int, g: int, b: int)
                     - Colour((r, g, b): tuple<int>)
                     - Colour(hex: str)
                     
                     Values of r,g,b must be in range[0, 255]
                     """

    def __init__(self, item):
        self.message = f"Not a valid colour: {item}\n" + self.default_message
        super(self.message)


class Colour:
    def __init__(self, *args, **kwargs):
        # If we have 3 arguments, interpret them as rgb values
        if len(args) >= 3:
            self.r, self.g, self.b = (clamp(int(x), 0, 255) for x in args[:3])
        elif "hex" in kwargs:
            self.r, self.g, self.b = Colour.hex_to_rgb(kwargs["hex"])
        elif "r" in kwargs and "g" in kwargs and "b" in kwargs:
            self.r, self.g, self.b = (
                clamp(int(kwargs["r"]), 0, 255),
                clamp(int(kwargs["g"]), 0, 255),
                clamp(int(kwargs["b"]), 0, 255),
            )
        elif isinstance(args[0], str):
            self.r, self.g, self.b = Colour.hex_to_rgb(args[0])
        elif isinstance(args[0], Iterable):
            self.r, self.g, self.b = (clamp(int(x), 0, 255) for x in args[0][:3])
        else:
            raise ColourConstructorError(args)

    @staticmethod
    def hex_to_rgb(hex_string):
        # Remove any leading '#' if present
        hex_string = hex_string.lstrip("#")

        # Check if the hex string is a valid length (it's always 6 characters long)
        if len(hex_string) != 6:
            raise ValueError("Invalid hex string length")  # noqa: TRY003

        # Convert the hex string to RGB values
        r = int(hex_string[0:2], 16)
        g = int(hex_string[2:4], 16)
        b = int(hex_string[4:6], 16)

        return r, g, b

    @staticmethod
    def rgb_to_hex(rgb_tuple):
        # Ensure that the RGB values are in the valid range (0-255)
        r, g, b = rgb_tuple
        if not (0 <= r <= 255) or not (0 <= g <= 255) or not (0 <= b <= 255):
            raise ValueError("RGB values must be in the range 0-255")  # noqa: TRY003

        # Convert the RGB values to a hex string
        return f"#{r:02X}{g:02X}{b:02X}"

    def as_rgb(self) -> tuple[int, int, int]:
        return self.r, self.g, self.b

    def as_hex(self) -> str:
        return Colour.rgb_to_hex(self.as_rgb())

    def __str__(self) -> str:
        return self.as_hex()

    def __repr__(self) -> str:
        return f"Colour({self.as_hex()})"


GREEN = Colour(0, 255, 0)
RED = Colour(255, 0, 0)
BLUE = Colour(0, 0, 255)
YELLOW = Colour(255, 255, 0)
HOT_ORANGE = Colour(255, 100, 0)


def get_linear_gradient_value(x, x_min, x_max, c_min: Colour, c_max: Colour) -> Colour:
    r = int(map_range(x, x_min, x_max, float(c_min.r), float(c_max.r)))
    g = int(map_range(x, x_min, x_max, float(c_min.g), float(c_max.g)))
    b = int(map_range(x, x_min, x_max, float(c_min.b), float(c_max.b)))
    return Colour(r, g, b)
