class UsersController < ApplicationController
  def new
    @user = User.new
  end

  def create
    @user = User.new(user_params)

    binding.pry
    
    if @user.save
        render text: "Success!"
    else
      flash = @user.errors
      redirect_to new_user_path
    end
  end

  def destroy
  end

private
  def user_params
    params.require(:user).permit(
      :first_name,
      :last_name,
      :password_digest,
      :github_username
    )
  end
end
