package utils;

public class Utils {
    
    public static double hypotenuse(double a, double b) {
        double r = 0;
        if (Math.abs(a) > Math.abs(b)) {
            r = b / a;
            double c = Math.abs(a) * Math.sqrt(1 + r * r);
            return c;
        }
        if (b != 0) {
            r = a / b;
            double c = Math.abs(b) * Math.sqrt(1 + r * r);
            return c;
        }
        return 0;
    }

    public static int nextPow2(int v) {
        if (v == 0) v = 1;
        v--;
        v |= v >>> 1;
        v |= v >>> 2;
        v |= v >>> 4;
        v |= v >>> 8;
        v |= v >>> 16;
        return v + 1;
    }
}