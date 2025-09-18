package ruse;

public class Student {
    public final String name;
    public final String surname;
    public final int age;
    public final int[] grades;

    public Student(String name, String surname, int age, int[] grades) {
        this.name = name;
        this.surname = surname;
        this.age = age;
        this.grades = grades;
    }
}