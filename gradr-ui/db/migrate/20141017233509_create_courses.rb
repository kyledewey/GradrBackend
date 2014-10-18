class CreateCourses < ActiveRecord::Migration
  def change
    create_table :courses do |t|
      t.string :title
    #   t.enrollment :int
    #   t.capacity :int
      t.timestamps
    end
  end
end
