public class DistanceUnit {
    public static final DistanceUnit INCH = new DistanceUnit(0.0254, "in", "inch");
    public static final DistanceUnit YARD = new DistanceUnit(0.9144, "yd", "yards");
    public static final DistanceUnit FEET = new DistanceUnit(0.3048, "ft", "feet");
    public static final DistanceUnit KILOMETERS = new DistanceUnit(1000.0, "km", "kilometers");
    public static final DistanceUnit NAUTICALMILES = new DistanceUnit(1852.0, "NM", "nauticalmiles");
    public static final DistanceUnit MILLIMETERS = new DistanceUnit(0.001, "mm", "millimeters");
    public static final DistanceUnit CENTIMETERS = new DistanceUnit(0.01, "cm", "centimeters");
    public static final DistanceUnit MILES = new DistanceUnit(1609.344, "mi", "miles");
    public static final DistanceUnit METERS = new DistanceUnit(1, "m", "meters");

    public final double meters;
    public final String shortName;
    public final String longName;

    public DistanceUnit(double meters, String shortName, String longName) {
        this.meters = meters;
        this.shortName = shortName;
        this.longName = longName;
    }

    public static double convert(double distance, DistanceUnit from, DistanceUnit to) {
        if (from == to) {
            return distance;
        } else {
            return distance * from.meters / to.meters;
        }
    }
}