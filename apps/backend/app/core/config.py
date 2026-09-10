from pydantic_settings import BaseSettings

class Settings(BaseSettings):
    PROJECT_NAME: str = "Genome Guides Backend"
    API_V1_STR: str = "/api/v1"
    VERSION: str = "0.1.0"
    
    class Config:
        env_file = ".env"

settings = Settings()
