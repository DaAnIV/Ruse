export class DistanceUnit {
    public static INCH: DistanceUnit = new DistanceUnit(0.0254, "in", "inch");
    public static YARD: DistanceUnit = new DistanceUnit(0.9144, "yd", "yards");
    public static FEET: DistanceUnit = new DistanceUnit(0.3048, "ft", "feet");
    public static KILOMETERS: DistanceUnit = new DistanceUnit(1000.0, "km", "kilometers");
    public static NAUTICALMILES: DistanceUnit = new DistanceUnit(1852.0, "NM", "nauticalmiles");
    public static MILLIMETERS: DistanceUnit = new DistanceUnit(0.001, "mm", "millimeters");
    public static CENTIMETERS: DistanceUnit = new DistanceUnit(0.01, "cm", "centimeters");
    public static MILES: DistanceUnit = new DistanceUnit(1609.344, "mi", "miles");
    public static METERS: DistanceUnit = new DistanceUnit(1, "m", "meters");

    public static convert(distance: number, from: DistanceUnit, to: DistanceUnit): number {
        if (from == to) {
            return distance;
        } else {
            return distance * from.meters / to.meters;
        }
    }

    constructor(public meters: number, public short: string, public long: string) {}
}
