class User < ActiveRecord::Base
  has_secure_password
  has_many :memberships

  attr_accessor :first_name,
                :last_name,
                :email,
                :github_username,
                :password,
                :password_confirmation
end
